//! Evaluation of the correction LLM on dictated French (ignored by default):
//!   PLUME_TEST_GGUF=/chemin/model.gguf cargo test --release --test e2e_llm_eval -- --ignored --nocapture   (+ --features cuda)
//! Twelve cases: six short sentences and three long paragraphs, each degraded (lowercase, no punctuation
//! or accents) then already punctuated by Whisper. Used to judge another model or a prompt change: see ADR 0004.
//! Minimum bar, measured on Qwen3-0.6B: never an empty output, a changed word in at most 4 of 12 cases (0 on GPU, 3 on CPU), at least 8 well-formed sentences out of 12.
mod common;
use common::*;
use plume::adapters::processor::LocalProcessor;
use plume::domain::ports::processor::TextProcessor;

const LONG: &[&str] = &[
 "Bonjour à tous, je voulais faire un point rapide sur l'avancement du projet. Nous avons terminé la migration de la base de données hier soir, et les premiers tests montrent une amélioration notable des temps de réponse. Il reste quelques ajustements à faire sur l'interface avant la mise en production prévue vendredi.",
 "Pour la réunion de jeudi, pouvez-vous préparer un résumé des ventes du dernier trimestre ? Marie s'occupera des chiffres de la région nord, et Pierre de ceux du sud. N'oubliez pas d'inclure les retours des clients, car la direction veut comprendre pourquoi le taux de satisfaction a baissé de cinq points.",
 "J'ai réservé une table pour six personnes au restaurant du coin, samedi soir à vingt heures. Ils proposent un menu végétarien, ce qui conviendra à Sophie. Si quelqu'un a des allergies, prévenez-moi avant mercredi pour que je puisse les informer.",
];

fn well_formed(s: &str) -> bool { s.chars().next().is_some_and(char::is_uppercase) && s.trim_end().ends_with(['.', '?', '!']) }

#[derive(Default, Clone)]
struct Stats { n: usize, formed: usize, empty: usize, changed: usize, ms: u128 }
impl Stats {
    fn add(&mut self, label: &str, input: String, out: String, ms: u128) {
        self.n += 1; self.ms += ms;
        if out.trim().is_empty() { self.empty += 1; println!("  [{label}] VIDE ({ms} ms)"); return; }
        if plain_words(&out) != plain_words(&input) { self.changed += 1; println!("  [{label}] MOTS CHANGÉS\n     in : {input}\n     out: {out}"); }
        if well_formed(&out) { self.formed += 1; }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn llm_keeps_every_word_and_fixes_the_form_of_french_dictations() {
    let path = std::env::var("PLUME_TEST_GGUF").expect("définir PLUME_TEST_GGUF");
    let llm = LocalProcessor::load(std::path::Path::new(&path), plume::config::style_prompt("clean", "fr")).unwrap();
    let mut st = Stats::default();
    println!("--- 6 phrases courtes dégradées");
    for (name, reference) in FRENCH_SENTENCES {
        let input = plain_words(reference).join(" ");
        let t = std::time::Instant::now();
        let out = llm.process(&input).await.unwrap();
        st.add(name, input, out, t.elapsed().as_millis());
    }
    let short = st.clone();
    println!("--- 3 paragraphes longs, forme dégradée puis forme ponctuée");
    for (i, text) in LONG.iter().enumerate() {
        for (form, input) in [("dégradé", plain_words(text).join(" ")), ("ponctué", text.to_string())] {
            let t = std::time::Instant::now();
            let out = llm.process(&input).await.unwrap();
            st.add(&format!("long{i} {form}"), input, out, t.elapsed().as_millis());
        }
    }
    println!("RÉSULTAT courtes : {}/{} bien formées, {} vides, {} mots changés, {} ms en moyenne", short.formed, short.n, short.empty, short.changed, short.ms / short.n as u128);
    println!("RÉSULTAT total   : {}/{} bien formées, {} vides, {} mots changés, {} ms en moyenne", st.formed, st.n, st.empty, st.changed, st.ms / st.n as u128);
    assert_eq!(st.empty, 0, "sortie vide : le modèle n'a pas répondu (réflexion trop longue ?)");
    // The dictation service keeps the raw text for a changed word (application::fidelity): this counts how often it would have to.
    // Measured on Qwen3-0.6B: 0 of 12 on GPU, 3 of 12 on CPU (e.g. fallu -> fallé).
    assert!(st.changed <= 4, "the model changed words in {} of {} cases", st.changed, st.n);
    assert!(st.formed >= 8, "seulement {}/{} phrases avec majuscule et ponctuation finale", st.formed, st.n);
}
