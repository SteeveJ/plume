/// Below this volume (RMS) the audio is noise or a barely audible voice: the phrases Whisper invents
/// on noise ("Thank you.", subtitle credits) are dropped. A real voice at the microphone is well above it.
pub const QUIET_RMS: f32 = 0.03;

/// Phrases Whisper `turbo` invents on noise, measured on white noise, a 50 Hz hum and key clicks
/// (English, French, automatic language), plus the known classics from its training data.
const INVENTED: &[&str] = &[
    "thank you", "thanks", "thank you for watching", "thanks for watching", "you", "bye", "bye bye",
    "merci", "merci à tous", "merci d avoir regardé cette vidéo", "au revoir", "à bientôt",
    "sous titrage société radio canada", "sous titrage st 501", "sous titres réalisés par la communauté d amara org",
    "subtitles by the amara org community",
];

pub fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

fn normalize(text: &str) -> String {
    // `to_lowercase` (not `to_ascii_lowercase`): "Merci À Tous" must become "merci à tous".
    let spaced: String = text.chars().map(|c| if c.is_alphanumeric() { c.to_lowercase().collect::<String>() } else { " ".into() }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True if `text` is not speech: punctuation alone (".", "...") whatever the volume,
/// or an invented phrase on weak audio. The same legitimate text said at normal volume passes,
/// and so does a quiet one when a voice detector confirmed speech (`speech_confirmed`, ADR 0014).
pub fn is_hallucination(text: &str, rms: f32, speech_confirmed: bool) -> bool {
    let norm = normalize(text);
    if !text.chars().any(|c| c.is_alphanumeric()) { return true; }
    !speech_confirmed && rms < QUIET_RMS && INVENTED.contains(&norm.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOISE: f32 = 0.01;
    const VOICE: f32 = 0.2;

    #[test]
    fn real_whisper_outputs_on_noise_are_dropped() {
        for t in ["Thank you.", "Thank You!", " thank you ", ".", "...", "Sous-titrage Société Radio-Canada",
                  "Merci.", "Subtitles by the Amara.org community", "you"] {
            assert!(is_hallucination(t, NOISE, false), "{t:?}");
        }
    }

    #[test]
    fn french_accents_and_capitals_are_matched() {
        for t in ["Merci À Tous !", "MERCI D'AVOIR REGARDÉ CETTE VIDÉO.", "À bientôt", "Sous-titres réalisés par la communauté d'Amara.org"] {
            assert!(is_hallucination(t, NOISE, false), "{t:?}");
        }
    }

    #[test]
    fn the_same_words_said_aloud_are_kept_except_pure_punctuation() {
        for t in ["Thank you.", "Merci.", "Au revoir !"] {
            assert!(!is_hallucination(t, VOICE, false), "{t:?}");
        }
        assert!(is_hallucination(".", VOICE, false), "de la ponctuation seule n'est jamais de la parole");
        assert!(is_hallucination("…", VOICE, false));
    }

    #[test]
    fn a_confirmed_voice_keeps_even_the_phrases_whisper_likes_to_invent() {
        assert!(!is_hallucination("Merci.", NOISE, true), "un « merci » très bas confirmé par le détecteur de voix");
        assert!(!is_hallucination("Thank you.", NOISE, true));
        assert!(is_hallucination(".", NOISE, true), "de la ponctuation seule reste écartée même confirmée");
    }

    #[test]
    fn quiet_but_real_sentences_are_kept() {
        for t in ["Ouvre le rapport de jeudi", "thank you for the report", "Merci Jean, à demain"] {
            assert!(!is_hallucination(t, NOISE, false), "{t:?}");
        }
    }

    #[test]
    fn rms_of_known_signals() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5, -0.5, 0.5, -0.5]) - 0.5).abs() < 1e-6);
        let sine: Vec<f32> = (0..16_000).map(|i| (i as f32 * std::f32::consts::TAU / 100.0).sin() * 0.3).collect();
        assert!((rms(&sine) - 0.3 / 2f32.sqrt()).abs() < 1e-3);
    }
}
