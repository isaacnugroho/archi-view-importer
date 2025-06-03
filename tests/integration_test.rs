use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_no_new_view_when_exists() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new()?;
    
    // Copy test2 to temp file
    let test2_path = PathBuf::from("test-files/test2.archimate");
    let temp_file = temp_dir.path().join("temp.archimate");
    fs::copy(&test2_path, &temp_file)?;
    
    // Run the application with test1 as source and temp file as target
    let test1_path = PathBuf::from("test-files/test1.archimate");
    // Run the application and simulate user input "\n" (empty selection)
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_archi-view-importer"))
        .arg(test1_path)
        .arg(&temp_file)
        .output()?;
    
    let output_str = String::from_utf8(output.stdout)?;
    let error_str = String::from_utf8(output.stderr)?;
    println!("=== STDOUT ===\n{}\n=== STDERR ===\n{}", output_str, error_str);
    
    // Check that the program found the views but didn't copy them due to empty selection
    assert!(output_str.contains("Views in source that don't exist in target"));
    assert!(output_str.contains("Default_Vew"));
    assert!(output_str.contains("Default View"));
    assert!(output_str.contains("No views selected for copying."));
    
    Ok(())
}
