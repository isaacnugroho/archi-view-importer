use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_interactive_view_selection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    
    let test2_path = PathBuf::from("tests/test2.archimate");
    let temp_file = temp_dir.path().join("temp.archimate");
    fs::copy(&test2_path, &temp_file)?;
    
    let test1_path = PathBuf::from("tests/test1.archimate");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_archi-view-importer"))
        .arg(test1_path)
        .arg(&temp_file)
        .output()?;
    
    let output_str = String::from_utf8(output.stdout)?;
    let error_str = String::from_utf8(output.stderr)?;
    println!("=== STDOUT ===\n{}\n=== STDERR ===\n{}", output_str, error_str);
    
    assert!(output_str.contains("Views in source that don't exist in target"));
    assert!(output_str.contains("Default View"));
    assert!(output_str.contains("Default_View"));
    assert!(output_str.contains("No views selected for copying."));
    
    Ok(())
}

#[test]
fn test_cli_view_selection_verbose() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    
    let test2_path = PathBuf::from("tests/test2.archimate");
    let temp_file = temp_dir.path().join("temp.archimate");
    fs::copy(&test2_path, &temp_file)?;
    
    let test1_path = PathBuf::from("tests/test1.archimate");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_archi-view-importer"))
        .arg(test1_path)
        .arg(&temp_file)
        .arg("--view")
        .arg("Default View")
        .arg("--verbose")
        .output()?;
    
    let output_str = String::from_utf8(output.stdout)?;
    let error_str = String::from_utf8(output.stderr)?;
    println!("=== STDOUT ===\n{}\n=== STDERR ===\n{}", output_str, error_str);
    
    assert!(output_str.contains("Views in source that don't exist in target"));
    assert!(output_str.contains("Default View"));
    assert!(output_str.contains("Default_View"));
    assert!(output_str.contains("Creating view Default View"));
    assert!(output_str.contains("Successfully imported views and elements into target file"));
    assert!(output_str.contains("Successfully copied:"));
    assert!(output_str.contains("- 1 view"));
    assert!(output_str.contains(".found element:"));
    assert!(output_str.contains(".found relation:"));
    assert!(output_str.contains(".new elements"));
    assert!(output_str.contains("creating element"));
    
    Ok(())
}

#[test]
fn test_cli_view_selection_non_verbose() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    
    let test2_path = PathBuf::from("tests/test2.archimate");
    let temp_file = temp_dir.path().join("temp.archimate");
    fs::copy(&test2_path, &temp_file)?;
    
    let test1_path = PathBuf::from("tests/test1.archimate");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_archi-view-importer"))
        .arg(test1_path)
        .arg(&temp_file)
        .arg("--view")
        .arg("Default View")
        .output()?;
    
    let output_str = String::from_utf8(output.stdout)?;
    let error_str = String::from_utf8(output.stderr)?;
    println!("=== STDOUT ===\n{}\n=== STDERR ===\n{}", output_str, error_str);
    
    assert!(output_str.contains("Creating view Default View"));
    assert!(!output_str.contains(".found element:"));
    assert!(!output_str.contains(".found relation:"));
    assert!(!output_str.contains(".new elements"));
    assert!(!output_str.contains("creating element"));
    
    Ok(())
}
