use std::{fs, path::PathBuf};

use clap::Parser;
use tqdm::tqdm;
use tree_sitter::TreeCursor;
use walkdir::WalkDir;

#[derive(Debug)]
pub enum StructForm {
    Unit,
    Tuple,
    Struct,
}

type StructInfo = (usize, usize, StructForm, String);

fn visit_vertical(source_code: &str, cursor: &mut TreeCursor, acc: &mut Vec<StructInfo>) {
    if cursor.goto_first_child() {
        visit_horizontal(source_code, cursor, acc);
        cursor.goto_parent();
    }
}

fn visit_horizontal(source_code: &str, cursor: &mut TreeCursor, acc: &mut Vec<StructInfo>) {
    loop {
        find_structs(source_code, cursor, acc);

        visit_vertical(source_code, cursor, acc);

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

pub fn find_structs(source_code: &str, cursor: &mut TreeCursor, acc: &mut Vec<StructInfo>) {
    let node = cursor.node();
    if node.kind() == "struct_item" {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let struct_name = node
            .child_by_field_name("name")
            .map(|n| n.utf8_text(&source_code.as_bytes()).unwrap().to_string())
            .unwrap_or_default();

        // avoid unicode-byte index mismatch problem
        // for example, "tests/ui/lint/lint-nonstandard-style-unicode-1.rs"
        // - just ignore them
        let source_chars: Vec<char> = source_code.chars().collect();
        if source_chars.len() <= end_byte - 1 { return; }

        let struct_form = determine_struct_form(source_code, cursor);

        let struct_info: StructInfo = (start_byte, end_byte, struct_form, struct_name);
        // dbg!(&struct_info);
        acc.push(struct_info);
    }
}

pub fn determine_struct_form(source_code: &str, cursor: &mut TreeCursor) -> StructForm {
    let node = cursor.node();
    let end_byte_idx = node.end_byte();
    // dbg!(start_byte_idx, end_byte_idx);

    let source_chars: Vec<char> = source_code.chars().collect();
    let target_char_1 = source_chars[end_byte_idx - 1];
    let target_char_2 = source_chars[end_byte_idx - 2];
    // dbg!(target_char_1, target_char_2);
    if target_char_1 == '}' {
        StructForm::Struct
    } else if target_char_2 == ')' {
        StructForm::Tuple
    } else {
        StructForm::Unit
    }
}

pub fn modify_structs(source_code: &str, structs: &Vec<StructInfo>) -> Vec<String> {
    let mut modified_versions = vec![source_code.to_string(); structs.len()]; // Initialize with the original code for each version

    for (i, &(start, end, ref form, ref name)) in structs.iter().enumerate() {
        for (version_index, version) in modified_versions.iter_mut().enumerate() {
            if version_index == i {
                let before = &source_code[..start];
                let after = &source_code[end..];
                let new_declaration = match form {
                    StructForm::Tuple => format!("struct {};", name),
                    _ => format!("struct {}();", name),
                };
                *version = format!("{}{}{}", before, new_declaration, after);
            }
        }
    }

    modified_versions
}

pub fn get_struct_crushed_sources(source_code: &str) -> Vec<String> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::language();
    parser.set_language(&language).unwrap();

    let tree = parser.parse(&source_code, None).unwrap();
    let mut found_structs: Vec<StructInfo> = Vec::new();
    visit_vertical(&source_code, &mut tree.walk(), &mut found_structs);

    modify_structs(&source_code, &found_structs)
}

// use clap cli parser
#[derive(Parser, Debug)]
struct Cli {
    #[arg(long)]
    input_file: Option<String>,
    #[arg(short, long)]
    input_dir: Option<String>,
    #[arg(short, long)]
    output_dir: Option<String>,
}

pub fn main() {
    let args = Cli::parse();

    let modified_sources: Vec<String> = if let Some(input_file) = args.input_file {
        let source_code = fs::read_to_string(input_file).unwrap();
        get_struct_crushed_sources(&source_code)
    } else if let Some(input_dir) = args.input_dir {
        let mut r: Vec<String> = vec![];
        for entry in tqdm(WalkDir::new(input_dir).into_iter()).style(tqdm::Style::Block) {
            let entry = entry.unwrap();
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if path.is_file() && ext.to_string_lossy() == "rs" {
                    // dbg!(path);
                    let source_code = fs::read_to_string(path).unwrap();
                    r.append(&mut get_struct_crushed_sources(&source_code));
                }
            }
        }
        r
    } else {
        panic!("No input file or directory provided");
    };

    println!("Number of generated files: {}", modified_sources.len());

    let output_dir: PathBuf = if let Some(o) = args.output_dir {
        // if directory exists then use it, otherwise create it (and notice it to the user)
        if !PathBuf::from(&o).exists() {
            fs::create_dir_all(&o).unwrap();
            println!("Created output directory: {}", o);
        }
        o.into()
    } else {
        // notice it uses current dir to user
        let current_dir = std::env::current_dir().unwrap();
        println!(
            "No output directory provided, using current directory: {:?}",
            current_dir
        );
        current_dir
    };

    for (idx, src) in modified_sources.iter().enumerate() {
        let file_name = format!("crushed_{}.rs", idx.to_string());
        let file_path = output_dir.join(file_name);
        fs::write(file_path, src).unwrap();
    }
}
