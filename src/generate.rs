use std::fs::File;
use std::io::{self, BufWriter, Write};

#[derive(Debug, Clone)]
pub struct ReplacePair(pub char, pub char);

impl ReplacePair {
    fn from(&self) -> char {
        self.0
    }
    pub fn to(&self) -> char {
        self.1
    }
}

pub fn generate_wordlist(
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
