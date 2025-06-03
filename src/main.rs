mod file_descriptor;

use crate::file_descriptor::FileDescriptor;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::{self, Write};
use std::str::FromStr;
use std::{env, process};
use xot::{output, Node, Xot};

struct ArchiModel<'a> {
    xot: &'a mut Xot,
    doc: Node,
    root: Node,
    view_map: HashMap<String, ElementInfo>,
    element_map: HashMap<String, ElementInfo>,
}

#[derive(Debug, Clone)]
struct ElementInfo {
    id: String,
    name: String,
    xml_string: String,
    folder_path: Vec<FolderInfo>,
}

#[derive(Debug, Clone)]
struct MissingElementInfo {
    id: String,
    name: String,
    folder_path: Vec<FolderInfo>,
}

#[derive(Debug, Clone)]
struct FolderInfo {
    id: String,
    name: String,
}

impl Borrow<str> for FolderInfo {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

impl Borrow<str> for &FolderInfo {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <source_archi_file> <target_archi_file>", args[0]);
        process::exit(1);
    }

    let source_file = &args[1];
    let target_file = &args[2];

    println!("-+ Analyzing Archi files");
    println!(" +- Source: {}", source_file);
    println!(" +- Target: {}", target_file);

    let source_descriptor = match FileDescriptor::from_path(source_file) {
        Ok(file_descriptor) => file_descriptor,
        Err(e) => {
            eprintln!("Error reading source file: {}", e);
            process::exit(1);
        }
    };

    let target_descriptor = match FileDescriptor::from_path(target_file) {
        Ok(file_descriptor) => file_descriptor,
        Err(e) => {
            eprintln!("Error reading target file: {}", e);
            process::exit(1);
        }
    };

    let source_content = match source_descriptor.read_xml() {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading source file: {}", e);
            process::exit(1);
        }
    };

    let target_content = match target_descriptor.read_xml() {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading target file: {}", e);
            process::exit(1);
        }
    };

    // Load and parse files
    let mut source_xot = Xot::new();
    let mut source = load_model(&mut source_xot, &source_content)?;
    let mut target_xot = Xot::new();
    let mut target = load_model(&mut target_xot, &target_content)?;

    // Find views in source that don't exist in target
    let missing_views = find_missing_views(&source, &target);

    if missing_views.is_empty() {
        println!("No new views to copy from source to target.");
        return Ok(());
    }
    // print!("source ids:");
    // for key in source.element_map.keys() {
    //     print!(" {}", key);
    // }
    // println!();

    // Display missing views with numbering
    println!("\nViews in source that don't exist in target:");
    for (i, view) in missing_views.iter().enumerate() {
        let folder_path = view.folder_path.join(" > ");
        println!("[{}] {} (in folder: {})", i + 1, view.name, folder_path);
    }

    // Get user selection
    let selection =
        get_input("\nEnter view numbers to copy (e.g., 1,3,5-7 or 'all' for all views): ")?;
    let selected_indices = parse_selection(&selection, missing_views.len())?;

    if selected_indices.is_empty() {
        println!("No views selected for copying.");
        return Ok(());
    }

    // Copy selected views and related elements
    let mut copied_views = 0;
    let mut copied_elements = 0;
    let mut copied_relations = 0;

    for &idx in &selected_indices {
        let view = &missing_views[idx - 1]; // Convert to 0-based index
        let (view_count, element_count, relation_count) =
            copy_view_with_elements(&mut source, &mut target, view)?;
        copied_views += view_count;
        copied_elements += element_count;
        copied_relations += relation_count;
    }

    let modified_target = target.xot.serialize_xml_string(
        output::xml::Parameters {
            declaration: Some(output::xml::Declaration {
                encoding: Some("UTF-8".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        },
        target.doc,
    )?;
    match target_descriptor.write_xml(&modified_target) {
        Ok(_) => println!("✅ Successfully imported views and elements into target file."),
        Err(e) => {
            eprintln!("❌ Error writing to target file: {}", e);
            process::exit(1);
        }
    }
    // save_model(&target, &target_path)?;

    println!("\nSuccessfully copied:");
    println!("- {} views", copied_views);
    println!("- {} elements", copied_elements);
    println!("- {} relations", copied_relations);
    // println!("Updated target file saved to: {}", target_path);
    Ok(())
}

fn get_input(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn load_model<'a>(
    xot: &'a mut Xot,
    content: &'a str,
) -> Result<ArchiModel<'a>, Box<dyn std::error::Error>> {
    let doc = xot.parse(content)?;
    let root = xot.root(doc);
    let mut model = ArchiModel {
        xot,
        doc,
        root,
        view_map: HashMap::new(),
        element_map: HashMap::new(),
    };

    extract_elements(&mut model)?;
    Ok(model)
}

fn extract_elements(model: &mut ArchiModel) -> Result<(), Box<dyn std::error::Error>> {
    let root = model.xot.first_child(model.root).unwrap();

    // Function to traverse folders recursively
    fn traverse_folders(
        xot: &Xot,
        node: Node,
        current_path: Vec<FolderInfo>,
        elements: &mut HashMap<String, ElementInfo>,
        views: &mut HashMap<String, ElementInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_path_info = &current_path.clone();
        for child in xot.children(node) {
            if !xot.is_element(child) {
                continue;
            }
            if xot.get_element_name(child) == xot.name("element").unwrap() {
                if let Some(xsi_type) = xot.get_attribute(
                    child,
                    xot.name_ns(
                        "type",
                        xot.namespace("http://www.w3.org/2001/XMLSchema-instance")
                            .unwrap(),
                    )
                    .unwrap(),
                ) {
                    let id = xot
                        .get_attribute(child, xot.name("id").unwrap())
                        .unwrap()
                        .to_string();
                    let name = xot
                        .get_attribute(child, xot.name("name").unwrap())
                        .unwrap_or("")
                        .to_string();
                    let xml_string = xot.serialize_xml_string(Default::default(), child)?;
                    if xsi_type.ends_with("ArchimateDiagramModel") {
                        views.insert(
                            id.clone(),
                            ElementInfo {
                                id,
                                name,
                                xml_string,
                                folder_path: current_path_info.clone(),
                            },
                        );
                    } else {
                        elements.insert(
                            id.clone(),
                            ElementInfo {
                                id,
                                name,
                                xml_string,
                                folder_path: current_path_info.clone(),
                            },
                        );
                    }
                }
            } else if xot.get_element_name(child) == xot.name("folder").unwrap() {
                let name =
                    String::from_str(xot.get_attribute(child, xot.name("name").unwrap()).unwrap())
                        .unwrap();
                // let id = format!("id-{}", uuid::Uuid::new_v4());
                let id =
                    String::from_str(xot.get_attribute(child, xot.name("id").unwrap()).unwrap())
                        .unwrap();
                let mut new_path = current_path_info.clone();
                let folder_info = FolderInfo { id, name };
                new_path.push(folder_info);
                traverse_folders(xot, child, new_path, elements, views)?;
            }
        }
        Ok(())
    }

    // Start traversal from the root
    let mut elements = HashMap::new();
    let mut views = HashMap::new();
    for child in model
        .xot
        .children(root)
        .filter(|&n| model.xot.is_element(n))
    {
        let element = model.xot.element(child).unwrap();
        // && model.xot.get_attribute(child, model.xot.name("type").unwrap())
        //     == Some("diagrams")
        if element.name() == model.xot.name("folder").unwrap() {
            let name = String::from_str(
                model
                    .xot
                    .get_attribute(child, model.xot.name("name").unwrap())
                    .unwrap(),
            )
            .unwrap();
            // let id = format!("id-{}", uuid::Uuid::new_v4());
            let id = String::from_str(
                model
                    .xot
                    .get_attribute(child, model.xot.name("id").unwrap())
                    .unwrap(),
            )
            .unwrap();
            let mut new_path = vec![];
            let folder_info = FolderInfo { id, name };
            // println!("{}", folder_info.name);
            new_path.push(folder_info);
            traverse_folders(model.xot, child, new_path, &mut elements, &mut views)?;
        }
    }
    model.element_map = elements;
    model.view_map = views;
    Ok(())
}

fn find_missing_views(source: &ArchiModel, target: &ArchiModel) -> Vec<MissingElementInfo> {
    let mut missing = Vec::new();

    for (id, view_info) in &source.view_map {
        if !target.view_map.contains_key(id) {
            missing.push(MissingElementInfo {
                id: view_info.id.clone(),
                name: view_info.name.clone(),
                folder_path: view_info.folder_path.clone(),
            });
        }
    }

    missing
}

fn parse_selection(
    input: &str,
    max_count: usize,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    let mut selected = HashSet::new();

    if input.trim().to_lowercase() == "all" {
        return Ok((1..=max_count).collect());
    }

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if part.contains('-') {
            // Range selection
            let range: Vec<&str> = part.split('-').collect();
            if range.len() == 2 {
                let start: usize = range[0].trim().parse()?;
                let end: usize = range[1].trim().parse()?;

                if start > end || start == 0 || end > max_count {
                    return Err(format!("Invalid range: {}-{}", start, end).into());
                }

                for i in start..=end {
                    selected.insert(i);
                }
            }
        } else {
            // Single number
            let num: usize = part.parse()?;
            if num == 0 || num > max_count {
                return Err(format!("Invalid view number: {}", num).into());
            }
            selected.insert(num);
        }
    }

    // Convert to sorted vector
    let mut result: Vec<usize> = selected.into_iter().collect();
    result.sort();

    Ok(result)
}

fn copy_view_with_elements(
    source: &mut ArchiModel,
    target: &mut ArchiModel,
    view: &MissingElementInfo,
) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
    let source_info = source.view_map.get(&view.id).unwrap();
    let view_node = target.xot.parse_fragment(source_info.xml_string.as_str())?;
    println!("Creating view {}", view.name);

    // Extract referenced elements and relations from the view
    let mut referenced_elements = HashSet::new();
    let mut referenced_relations = HashSet::new();

    fn extract_references(
        xot: &Xot,
        node: Node,
        elements: &mut HashSet<String>,
        relations: &mut HashSet<String>,
    ) {
        // Check if this node references any elements
        if let Some(element_ref) = xot.get_attribute(node, xot.name("archimateElement").unwrap()) {
            println!(".found element: {}", element_ref);
            elements.insert(element_ref.to_string());
        }

        // Check if this node references any relations
        if let Some(relation_ref) =
            xot.get_attribute(node, xot.name("archimateRelationship").unwrap())
        {
            println!(".found relation: {}", relation_ref);
            relations.insert(relation_ref.to_string());
        }

        // Recursively check children
        for child in xot.children(node).filter(|&n| xot.is_element(n)) {
            extract_references(xot, child, elements, relations);
        }
    }

    // Extract all referenced elements and relations from the view
    extract_references(
        target.xot,
        view_node,
        &mut referenced_elements,
        &mut referenced_relations,
    );

    // Filter to only those not already in target
    let new_elements: Vec<_> = referenced_elements
        .iter()
        .filter(|id| !target.element_map.contains_key(*id))
        .cloned()
        .collect();

    let new_relations: Vec<_> = referenced_relations
        .iter()
        .filter(|id| !target.element_map.contains_key(*id))
        .cloned()
        .collect();

    // Now copy elements to target
    for element_id in &new_elements {
        println!(".new elements {}", element_id);
        insert_new_element(source, target, element_id)?;
    }

    // Copy relations to target
    for element_id in &new_relations {
        println!(".new relations {}", element_id);
        insert_new_element(source, target, element_id)?;
    }

    insert_new_view(source, target, &view.id)?;

    // Return counts of what was copied
    Ok((1, new_elements.len(), new_relations.len()))
}

fn insert_new_element(
    source: &mut ArchiModel,
    target: &mut ArchiModel,
    element_id: &String,
) -> Result<(), Box<dyn Error>> {
    if !source.element_map.contains_key(element_id) {
        println!(".Not found in source {}", element_id);
    }
    if let Some(source_element_info) = source.element_map.get(element_id) {
        let target_element_folder =
            recursive_find_or_create_folder_path(target, &source_element_info.folder_path)?;

        // Clone the element
        println!("creating element {}", source_element_info.xml_string);
        let cloned_node = target.xot.parse(source_element_info.xml_string.as_str())?;
        let cloned_element = target.xot.document_element(cloned_node)?;
        // let cloned_element = target.xot.parse_fragment(source_element_info.xml_string.as_str())?;

        // Add to target folder
        target.xot.append(target_element_folder, cloned_element)?;
        target
            .element_map
            .insert(element_id.clone(), source_element_info.clone());
    }
    Ok(())
}

fn insert_new_view(
    source: &mut ArchiModel,
    target: &mut ArchiModel,
    element_id: &String,
) -> Result<(), Box<dyn Error>> {
    if let Some(source_element_info) = source.view_map.get(element_id) {
        let target_element_folder =
            recursive_find_or_create_folder_path(target, &source_element_info.folder_path)?;

        // Clone the element
        println!("creating view {}", source_element_info.xml_string);
        let cloned_node = target.xot.parse(source_element_info.xml_string.as_str())?;
        let cloned_element = target.xot.document_element(cloned_node)?;

        // Add to target folder
        target.xot.append(target_element_folder, cloned_element)?;

        target
            .element_map
            .insert(element_id.clone(), source_element_info.clone());
    }
    Ok(())
}

fn find_or_create_folder(
    model: &mut ArchiModel,
    folder_type: &str,
) -> Result<Node, Box<dyn std::error::Error>> {
    let root = model.xot.first_child(model.root).unwrap();

    // Look for existing folder with the given type
    for child in model
        .xot
        .children(root)
        .filter(|&n| model.xot.is_element(n))
    {
        let element = model.xot.element(child).unwrap();
        if element.name() == model.xot.name("folder").unwrap()
            && model
                .xot
                .get_attribute(child, model.xot.name("type").unwrap())
                == Some(folder_type)
        {
            return Ok(child);
        }
    }

    // If not found, create a new folder
    let folder_node = model.xot.new_element(model.xot.name("folder").unwrap());
    model
        .xot
        .set_attribute(folder_node, model.xot.name("type").unwrap(), folder_type);
    model.xot.set_attribute(
        folder_node,
        model.xot.name("id").unwrap(),
        format!("id-{}", uuid::Uuid::new_v4()),
    );

    // Add some default name based on type
    let name = match folder_type {
        "business" => "Business",
        "application" => "Application",
        "technology" => "Technology & Physical",
        "strategy" => "Strategy",
        "motivation" => "Motivation",
        "implementation_migration" => "Implementation & Migration",
        "relations" => "Relations",
        "diagrams" => "Views",
        _ => "Other",
    };
    model
        .xot
        .set_attribute(folder_node, model.xot.name("name").unwrap(), name);

    // Add to root
    model.xot.append(root, folder_node)?;

    Ok(folder_node)
}

fn recursive_find_or_create_folder_path(
    model: &mut ArchiModel,
    folder_path: &[FolderInfo],
) -> Result<Node, Box<dyn std::error::Error>> {
    if folder_path.is_empty() {
        // If no path specified, use the default diagrams folder
        return find_or_create_folder(model, "diagrams");
    }

    // First make sure the diagrams root folder exists
    // let mut current = find_or_create_folder(model, "diagrams")?;
    let mut current = model.xot.first_child(model.root).unwrap();
    // Now create or find each subfolder in the path
    for folder_info in folder_path {
        // Look for existing subfolder
        let mut found = false;
        let mut next_folder = None;
        let info_name = folder_info.name.clone();
        let folder_name = info_name.as_str();
        let info_id = folder_info.id.clone();
        let id = info_id.as_str();

        for child in model
            .xot
            .children(current)
            .filter(|&n| model.xot.is_element(n))
        {
            let element = model.xot.element(child).unwrap();
            if element.name() == model.xot.name("folder").unwrap()
                && model
                    .xot
                    .get_attribute(child, model.xot.name("name").unwrap())
                    == Some(folder_name)
            {
                found = true;
                next_folder = Some(child);
                break;
            }
        }

        if found {
            current = next_folder.unwrap();
        } else {
            // Create new subfolder
            let new_folder = model.xot.new_element(model.xot.name("folder").unwrap());
            model
                .xot
                .set_attribute(new_folder, model.xot.name("name").unwrap(), folder_name);
            model
                .xot
                .set_attribute(new_folder, model.xot.name("id").unwrap(), id);
            model.xot.append(current, new_folder)?;
            current = new_folder;
        }
    }

    Ok(current)
}
