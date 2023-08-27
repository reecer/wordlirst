mod generate;

use clap::Parser;
use generate::{generate_wordlist, ReplacePair};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek};
use std::path::PathBuf;

/// Simple program to generate a wordlist
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output file to write to
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    output: PathBuf,

    /// Input wordlist to permutate
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    dictionary: Option<PathBuf>,

    /// Custom terms to add to dictionary words
    #[arg(short, long, value_delimiter = ',')]
    terms: Vec<String>,

    /// Characters to replace (i.e. e=3 to replace e's with 3's)
    #[arg(short, long, value_parser=parse_key_val, default_value="o=0,e=3,l=1,i=!,a=@,s=$,t=7", value_delimiter=',')]
    replacements: Vec<ReplacePair>,

    /// Min/max password length
    #[arg(short, long, num_args = 2, default_values_t = vec![3, 6])]
    length: Vec<u8>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    // println!("{:#?}", args);

    // parse args
    let min_length = args.length[0] as usize;
    let max_length = args.length[1] as usize;
    let transforms = &args.replacements;
    let terms: Vec<String> = args
        .terms
        .into_iter()
        .filter(|word| word.len() <= max_length)
        .collect();
    let dictionary: Vec<String> = if let Some(dict_fname) = args.dictionary {
        read_dictionary(dict_fname, max_length)?
    } else {
        Vec::new()
    };

    // Print info
    println!("Dictionary length: {}", dictionary.len());
    println!("Terms length: {}", terms.len());
    println!("Replacements: {}", transforms.len());
    println!("Word min/max: {} - {}", min_length, max_length);

    // open for read/write/create
    let mut fout: File = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(args.output)?;

    // generate 'em
    println!("Generating...");
    generate_wordlist(&fout, dictionary, terms, transforms, min_length, max_length)?;

    // count resulting lines
    fout.seek(std::io::SeekFrom::Start(0))?;
    let line_count = BufReader::new(&fout).lines().count();
    println!("{} words generated!", line_count);

    Ok(())
}

fn read_dictionary(filename: PathBuf, max_length: usize) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    let lines = io::BufReader::new(file).lines();

    let dictionary: Vec<String> = lines
        .filter_map(|line| line.ok())
        .map(|word| word.trim().to_string())
        .filter(|word| word.len() > 0 && word.len() <= max_length)
        .collect();

    Ok(dictionary)
}

/// Parse a single key-value pair
fn parse_key_val(s: &str) -> Result<ReplacePair, Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok(ReplacePair(s[..pos].parse()?, s[pos + 1..].parse()?))
}
