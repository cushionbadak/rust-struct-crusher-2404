use std::{fs, path::PathBuf};

use clap::Parser;
use tqdm::tqdm;
use tree_sitter::TreeCursor;
use walkdir::WalkDir;

type TypePosInfo = (usize, usize, String);

fn visit_vertical(source_code: &str, cursor: &mut TreeCursor, acc: &mut Vec<TypePosInfo>) {
    if cursor.goto_first_child() {
        visit_horizontal(source_code, cursor, acc);
        cursor.goto_parent();
    }
}

fn visit_horizontal(source_code: &str, cursor: &mut TreeCursor, acc: &mut Vec<TypePosInfo>) {
    loop {
        find_type(source_code, cursor, acc);

        visit_vertical(source_code, cursor, acc);

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

pub fn find_type(source_code: &str, cursor: &mut TreeCursor, acc: &mut Vec<TypePosInfo>) {
    let node = cursor.node();
    match node.kind() {
        "type_identifier" => {
            let start_byte = node.start_byte();
            let end_byte = node.end_byte();

            let struct_name = node.to_string();
            dbg!(&struct_name);

            // avoid unicode-byte index mismatch problem
            // - just ignore them
            let source_chars: Vec<char> = source_code.chars().collect();
            if source_chars.len() <= end_byte - 1 {
                return;
            }

            let type_info: TypePosInfo = (start_byte, end_byte, struct_name);
            acc.push(type_info);
        }
        _ => {} // Other node kinds can be handled as needed
    }
}

pub fn modify_types(source_code: &str, structs: &Vec<TypePosInfo>) -> Vec<String> {
    const NEW_EXPRS: [&str; 4] = ["", "i32", "str", "Copy"];
    const SORTS: usize = NEW_EXPRS.len();
    let mut modified_versions = vec![source_code.to_string(); structs.len() * SORTS]; // Initialize with the original code for each version

    for (i, &(start, end, ref _name)) in structs.iter().enumerate() {
        let before = &source_code[..start];
        let after = &source_code[end..];

        for (j, n) in NEW_EXPRS.iter().enumerate() {
            modified_versions[i * SORTS + j] = format!("{}{}{}", before, n, after);
        }
    }

    modified_versions
}

pub fn get_struct_crushed_sources(source_code: &str) -> Vec<String> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::language();
    parser.set_language(&language).unwrap();

    let tree = parser.parse(&source_code, None).unwrap();
    let mut found_structs: Vec<TypePosInfo> = Vec::new();
    visit_vertical(&source_code, &mut tree.walk(), &mut found_structs);

    modify_types(&source_code, &found_structs)
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
