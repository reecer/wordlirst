use clap::Parser;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Seek, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct ReplacePair(char, char);

impl ReplacePair {
    fn from(&self) -> char {
        self.0
    }
    fn to(&self) -> char {
        self.1
    }
}

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

    // inaccurate word count estimation
    let estimated_count = estimate_word_count(&dictionary, &terms, min_length, max_length);
    println!("Estimated word count: {}", estimated_count);

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

fn generate_wordlist(
    fout: &File,
    dictionary: Vec<String>,
    terms: Vec<String>,
    transforms: &Vec<ReplacePair>,
    min_length: usize,
    max_length: usize,
) -> io::Result<()> {
    let mut writer = BufWriter::new(fout);

    for word1 in &dictionary {
        // word
        if word1.len() >= min_length {
            generate_permutations(&mut writer, word1, transforms)?;
        }
        // word + word
        generate_concats(
            &mut writer,
            word1,
            &dictionary,
            transforms,
            min_length,
            max_length,
        )?;
        // word + term
        generate_concats(
            &mut writer,
            word1,
            &terms,
            transforms,
            min_length,
            max_length,
        )?;
    }

    for term1 in &terms {
        // term
        if term1.len() >= min_length {
            generate_permutations(&mut writer, term1, transforms)?;
        }
        // term + term
        generate_concats(
            &mut writer,
            term1,
            &terms,
            transforms,
            min_length,
            max_length,
        )?;
        // term + word
        generate_concats(
            &mut writer,
            term1,
            &dictionary,
            transforms,
            min_length,
            max_length,
        )?;
    }

    writer.flush()?;
    Ok(())
}

fn generate_concats(
    writer: &mut BufWriter<&File>,
    term: &String,
    terms: &Vec<String>,
    transforms: &Vec<ReplacePair>,
    min_length: usize,
    max_length: usize,
) -> io::Result<()> {
    if terms.len() <= 0 {
        return Ok(());
    }

    for term1 in terms {
        if term1.len() > max_length {
            continue;
        }

        // term + term
        let term_term = format!("{}{}", term, term1);
        if term_term.len() > max_length {
            continue;
        }

        if term_term.len() >= min_length {
            generate_permutations(writer, &term_term, transforms)?;
        }
        // recurse
        generate_concats(
            writer, &term_term, terms, transforms, min_length, max_length,
        )?;
    }

    writer.flush()?;
    Ok(())
}

// adds capitalization and transforms
fn generate_permutations(
    writer: &mut BufWriter<&File>,
    w: &String,
    replacements: &Vec<ReplacePair>,
) -> io::Result<()> {
    println!("{}", w);

    let word = w.to_lowercase();
    for i in 0..(1 << word.len()) {
        let mut combination = String::new();
        for (j, c) in word.chars().enumerate() {
            if (i >> j) & 1 == 1 {
                combination.push(c.to_ascii_uppercase());
            } else {
                combination.push(c);
            }
        }
        // for each caps, transform
        add_transformations(writer, combination.as_str(), replacements)?;
    }

    Ok(())
}

// "leet" transforms
fn add_transformations(
    writer: &mut BufWriter<&File>,
    word: &str,
    replacements: &Vec<ReplacePair>,
) -> io::Result<()> {
    let mut current = vec![String::new()];
    for c in word.chars() {
        let mut new_combinations = Vec::new();
        for combo in current.iter() {
            new_combinations.push(combo.clone() + &c.to_string());
            for r in replacements {
                if c == r.from() {
                    new_combinations.push(combo.clone() + &r.to().to_string());
                }
            }
        }
        current = new_combinations;
    }

    for combo in current {
        writer.write_all(combo.as_bytes())?;
        writer.write_all(b"\n")?;
    }

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

fn estimate_word_count(
    dictionary: &[String],
    terms: &[String],
    min_length: usize,
    max_length: usize,
) -> usize {
    let dictionary_size = dictionary.len();
    let term_count = terms.len() + 1; // Including the original word
    let lengths_in_range = (min_length..=max_length).count();

    // Assuming the worst-case scenario where all permutations and transformations are taken
    // into account for each word, which would lead to an overestimation
    let estimated_count =
        dictionary_size * term_count * lengths_in_range * 3_usize.pow(max_length as u32);

    estimated_count
}

/// Parse a single key-value pair
fn parse_key_val(s: &str) -> Result<ReplacePair, Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok(ReplacePair(s[..pos].parse()?, s[pos + 1..].parse()?))
}
