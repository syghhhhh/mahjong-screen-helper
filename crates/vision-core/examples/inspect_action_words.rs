//! Local-only action input/logit diagnostic, with no overlay activation.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs::File, io::Write, path::Path, time::Instant};
    use vision_core::{
        RgbImage,
        action_input::{
            ActionWordInput, COMPLETE_COMPONENT_PREPROCESSING_ID, INPUT_FLOATS, PREPROCESSING_ID,
            prepare_action_word, prepare_complete_action_word,
        },
        neural::WinMlActionSession,
    };
    let args: Vec<_> = env::args().collect();
    let complete = args
        .get(1)
        .is_some_and(|value| value == "--complete-components");
    let args = if complete { &args[2..] } else { &args[1..] };
    if !(3..=258).contains(&args.len()) {
        return Err("usage: inspect_action_words [--complete-components] actions.onnx new-inputs.f32le word.png [word.png ...] (1..256)".into());
    }
    println!(
        "preprocessing: {}",
        if complete {
            COMPLETE_COMPONENT_PREPROCESSING_ID
        } else {
            PREPROCESSING_ID
        }
    );
    let mut file = File::create_new(&args[1])?;
    let start = Instant::now();
    let session = WinMlActionSession::load(Path::new(&args[0]))?;
    println!(
        "model_load_ms: {:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    let mut preparation = 0.0;
    let mut evaluation = 0.0;
    for (index, path) in args[2..].iter().enumerate() {
        let start = Instant::now();
        let image = RgbImage::read_png(Path::new(path))?;
        let word = if complete {
            prepare_complete_action_word(&image)?
        } else {
            prepare_action_word(&image)?
        };
        let input = match word {
            ActionWordInput::Empty => {
                println!("word {index}: Empty");
                vec![0.0; INPUT_FLOATS]
            }
            ActionWordInput::Unlocalized => {
                println!("word {index}: Unlocalized");
                vec![0.0; INPUT_FLOATS]
            }
            ActionWordInput::Prepared(word) => {
                println!(
                    "word {index}: {:?}; foreground={}",
                    word.bounds, word.foreground_pixels
                );
                word.pixels
            }
        };
        preparation += start.elapsed().as_secs_f64() * 1000.0;
        for value in &input {
            file.write_all(&value.to_le_bytes())?;
        }
        let start = Instant::now();
        let logits = session.evaluate(&input)?;
        evaluation += start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "logits {index}: {}",
            logits
                .iter()
                .map(|v| format!("{v:.9}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    file.flush()?;
    println!("preparation_total_ms: {preparation:.3}");
    println!("evaluation_total_ms: {evaluation:.3}");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Native Windows ML diagnostic requires Windows 10 version 1809 or newer");
}
