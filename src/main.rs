// use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

// TODO: allow multiple replacements (i.e. i => 1,!)
const LETTER_REPLACEMENTS: [(char, char); 7] = [
    ('e', '3'),
    ('o', '0'),
    ('i', '!'),
    ('a', '@'),
    ('s', '$'),
    ('t', '7'),
    ('l', '1'),
];

fn main() -> io::Result<()> {
    // let args: Vec<String> = env::args().collect();

    // if args.len() < 4 {
    //     println!("Usage: {} <min_length> <max_length> <term> [term2 ...]", args[0]);
    //     return;
    // }

    // let min_length: usize = args[1].parse().unwrap_or(0);
    // let max_length: usize = args[2].parse().unwrap_or(std::usize::MAX);
    // let terms: Vec<String> = args[3..].to_vec();

    let min_length: usize = 1;
    let max_length: usize = 17;
    let mut terms: Vec<String> = Vec::new();
    terms.push("matsu".to_string());
    terms.push("borough".to_string());
    // terms.push("matsuborough".to_string());

    // let mut dictionary: Vec<String> = Vec::new();
    // dictionary.push("world".to_string());
    // dictionary.push("rust".to_string());
    // dictionary.push("programming".to_string());

    // let dict = File::open(dictionary)?;

    // if let Ok(dictionary) = read_dictionary("dictionary.txt") {
    if dictionary.len() >= 0 {
        let estimated_count = estimate_word_count(&dictionary, &terms, min_length, max_length);
        println!("Estimated word count: {}", estimated_count);

        let fout = File::create("wordlist.txt")?;
        let mut writer = BufWriter::new(fout);

        for word1 in &dictionary {
            // word too long
            if word1.len() > max_length {
                continue;
            }

            // word is long enough
            if word1.len() >= min_length {
                generate_permutations(&mut writer, word1.clone())?;
            }

            // word + term
            // term + word
            for term in &terms {
                let term_word = format!("{}{}", term, word1);
                let word_term = format!("{}{}", word1, term);

                if term_word.len() >= min_length && term_word.len() <= max_length {
                    generate_permutations(&mut writer, term_word.clone())?;
                    generate_permutations(&mut writer, word_term.clone())?;
                }
            }

            writer.flush()?;
        }

        // Add terms even if dictionary is empty
        for term1 in &terms {
            if term1.len() > max_length {
                continue;
            }

            // term
            if term1.len() >= min_length {
                generate_permutations(&mut writer, term1.clone())?;
            }

            // term + term
            for term2 in &terms {
                let term_term = format!("{}{}", term1, term2);
                if term_term.len() >= min_length && term_term.len() <= max_length {
                    generate_permutations(&mut writer, term_term.clone())?;
                }
            }

            writer.flush()?;
        }
    } else {
        println!("Error reading dictionary file.");
    }

    Ok(())
}

// pub fn generate_wordlist(
//     out_fname: &str,
//     dict_file: BufReader,
//     terms: &[&str],
//     transforms: &[(char, char)],
//     max_length: u8,
//     min_length: u8,
// ) -> io::Result<u64> {
//     let mut count: u64 = 0;

//     Ok(count)
// }

// adds capitalization and transforms
fn generate_permutations(writer: &mut BufWriter<File>, w: String) -> io::Result<()> {
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
        add_transformations(writer, combination.as_str())?;
    }

    Ok(())
}

// "leet" transforms
fn add_transformations(writer: &mut BufWriter<File>, word: &str) -> io::Result<()> {
    let mut current = vec![String::new()];
    for c in word.chars() {
        let mut new_combinations = Vec::new();
        for combo in current.iter() {
            new_combinations.push(combo.clone() + &c.to_string());
            for &(from, to) in &LETTER_REPLACEMENTS {
                if c == from {
                    new_combinations.push(combo.clone() + &to.to_string());
                }
            }
        }
        current = new_combinations;
    }

    for combo in current {
        writer.write_all(combo.as_bytes())?;
        writer.write_all(b"\n")?;
        // writeln!(fout, "{}", combo)?;
    }

    Ok(())
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

fn read_dictionary(filename: &str) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    let lines = io::BufReader::new(file).lines();

    let dictionary: Vec<String> = lines
        .filter_map(|line| line.ok())
        .map(|word| word.trim().to_string())
        .collect();

    Ok(dictionary)
}
