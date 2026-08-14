//! Privacy Lens detection layer, built on `pii-vault`.
//!
//! Rebost records categories and counts, never the sensitive values:
//! email, phone, iban, nif, nie, credit_card, ip_address.

use pii_vault::{
    recognizer::{PatternDef, RecognizerDef, RegexRecognizer},
    Analyzer, Anonymizer, EntityType, Operator, Recognizer, RecognizerResult,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Rebost-facing PII summary stored on Cards.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PiiSummary {
    pub total: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub categories: BTreeMap<String, u32>,
}

impl PiiSummary {
    pub fn merge(&mut self, other: &PiiSummary) {
        self.total += other.total;
        for (k, v) in &other.categories {
            *self.categories.entry(k.clone()).or_insert(0) += v;
        }
    }
}

const SCORE_THRESHOLD: f64 = 0.5;

/// Map a pii-vault entity type to the Rebost category key.
fn category_of(entity: &str) -> Option<&'static str> {
    match entity {
        "EMAIL_ADDRESS" => Some("email"),
        "PHONE_NUMBER" => Some("phone"),
        "IBAN_CODE" => Some("iban"),
        "ES_NIF" => Some("nif"),
        "ES_NIE" => Some("nie"),
        "CREDIT_CARD" => Some("credit_card"),
        "IP_ADDRESS" => Some("ip_address"),
        _ => None,
    }
}

/// Placeholder used by "Copy without personal information".
fn placeholder_of(entity: &str) -> &'static str {
    match entity {
        "EMAIL_ADDRESS" => "[EMAIL_ADDRESS]",
        "PHONE_NUMBER" => "[PHONE]",
        "IBAN_CODE" => "[IBAN]",
        "ES_NIF" => "[NIF]",
        "ES_NIE" => "[NIE]",
        "CREDIT_CARD" => "[CREDIT_CARD]",
        "IP_ADDRESS" => "[IP_ADDRESS]",
        _ => "[PERSONAL_INFORMATION]",
    }
}

fn regex_recognizer(
    name: &str,
    entity: &str,
    patterns: Vec<(&str, &str, f64)>,
    context_words: Vec<&str>,
    context_boost: f64,
    validators: Vec<&str>,
) -> RegexRecognizer {
    let def = RecognizerDef {
        name: name.to_string(),
        entity_type: entity.to_string(),
        version: "1".to_string(),
        patterns: patterns
            .into_iter()
            .map(|(n, re, score)| PatternDef {
                name: n.to_string(),
                regex: re.to_string(),
                score,
            })
            .collect(),
        context_words: context_words.into_iter().map(String::from).collect(),
        context_score_boost: context_boost,
        deny_list: Vec::new(),
        validators: validators.into_iter().map(String::from).collect(),
        supported_languages: None,
    };
    RegexRecognizer::from_def(def).expect("built-in recognizer regex must compile")
}

/// Spanish NIF (DNI + control letter), NIE and CIF with real checksum
/// validation — pii-vault has no built-in Spanish validators, so this is a
/// custom `Recognizer`.
struct SpanishIdRecognizer {
    entities: Vec<EntityType>,
    nif_re: regex::Regex,
    nie_re: regex::Regex,
    cif_re: regex::Regex,
}

const DNI_LETTERS: &[u8] = b"TRWAGMYFPDXBNJZSQVHLCKE";

fn nif_valid(candidate: &str) -> bool {
    let (digits, letter) = candidate.split_at(8);
    let Ok(number) = digits.parse::<u64>() else {
        return false;
    };
    let expected = DNI_LETTERS[(number % 23) as usize] as char;
    letter.starts_with(expected)
}

fn nie_valid(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    let prefix = match chars.next() {
        Some('X') => '0',
        Some('Y') => '1',
        Some('Z') => '2',
        _ => return false,
    };
    let rest: String = chars.collect();
    let (digits, letter) = rest.split_at(7);
    let Ok(number) = format!("{prefix}{digits}").parse::<u64>() else {
        return false;
    };
    let expected = DNI_LETTERS[(number % 23) as usize] as char;
    letter.starts_with(expected)
}

fn cif_valid(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() != 9 {
        return false;
    }
    let org = bytes[0] as char;
    let digits = &candidate[1..8];
    let control = bytes[8] as char;
    let Ok(_) = digits.parse::<u32>() else {
        return false;
    };
    let mut even_sum = 0u32;
    let mut odd_sum = 0u32;
    for (i, c) in digits.chars().enumerate() {
        let d = c.to_digit(10).unwrap();
        if i % 2 == 0 {
            // 1st, 3rd, 5th, 7th digit: double and add digits
            let dd = d * 2;
            odd_sum += dd / 10 + dd % 10;
        } else {
            even_sum += d;
        }
    }
    let total = even_sum + odd_sum;
    let control_digit = (10 - (total % 10)) % 10;
    let control_letter = b"JABCDEFGHI"[control_digit as usize] as char;
    match org {
        // Organisations whose control must be a letter
        'K' | 'P' | 'Q' | 'S' | 'N' | 'W' | 'R' => control == control_letter,
        // Organisations whose control must be a digit
        'A' | 'B' | 'E' | 'H' => control
            .to_digit(10)
            .map(|d| d == control_digit)
            .unwrap_or(false),
        // Everything else accepts both forms
        _ => {
            control
                .to_digit(10)
                .map(|d| d == control_digit)
                .unwrap_or(false)
                || control == control_letter
        }
    }
}

impl SpanishIdRecognizer {
    fn new() -> Self {
        Self {
            entities: vec![EntityType::new("ES_NIF"), EntityType::new("ES_NIE")],
            nif_re: regex::Regex::new(r"\b\d{8}[A-HJ-NP-TV-Z]\b").unwrap(),
            nie_re: regex::Regex::new(r"\b[XYZ]\d{7}[A-HJ-NP-TV-Z]\b").unwrap(),
            cif_re: regex::Regex::new(r"\b[ABCDEFGHJKLMNPQRSUVW]\d{7}[0-9A-J]\b").unwrap(),
        }
    }
}

impl Recognizer for SpanishIdRecognizer {
    fn name(&self) -> &str {
        "spanish_id"
    }

    fn supported_entities(&self) -> &[EntityType] {
        &self.entities
    }

    fn analyze(&self, text: &str, entities: &[EntityType]) -> Vec<RecognizerResult> {
        let want_nif = entities.is_empty() || entities.iter().any(|e| e.as_str() == "ES_NIF");
        let want_nie = entities.is_empty() || entities.iter().any(|e| e.as_str() == "ES_NIE");
        let mut out = Vec::new();
        if want_nif {
            for m in self.nif_re.find_iter(text) {
                if nif_valid(m.as_str()) {
                    out.push(RecognizerResult {
                        entity_type: EntityType::new("ES_NIF"),
                        start: m.start(),
                        end: m.end(),
                        score: 0.95,
                        recognizer_name: Some("es_nif".into()),
                    });
                }
            }
            // Company NIF (formerly CIF) counts under the nif category.
            for m in self.cif_re.find_iter(text) {
                if cif_valid(m.as_str()) {
                    out.push(RecognizerResult {
                        entity_type: EntityType::new("ES_NIF"),
                        start: m.start(),
                        end: m.end(),
                        score: 0.85,
                        recognizer_name: Some("es_cif".into()),
                    });
                }
            }
        }
        if want_nie {
            for m in self.nie_re.find_iter(text) {
                if nie_valid(m.as_str()) {
                    out.push(RecognizerResult {
                        entity_type: EntityType::new("ES_NIE"),
                        start: m.start(),
                        end: m.end(),
                        score: 0.95,
                        recognizer_name: Some("es_nie".into()),
                    });
                }
            }
        }
        out
    }
}

/// The Rebost PII scanner. Cheap to construct; hold one per app.
pub struct PiiScanner {
    analyzer: Analyzer,
}

impl Default for PiiScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl PiiScanner {
    pub fn new() -> Self {
        let recognizers: Vec<Box<dyn Recognizer>> = vec![
            Box::new(regex_recognizer(
                "email",
                "EMAIL_ADDRESS",
                vec![(
                    "email",
                    r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
                    0.9,
                )],
                vec![],
                0.0,
                vec![],
            )),
            Box::new(regex_recognizer(
                "phone",
                "PHONE_NUMBER",
                vec![
                    // International with prefix: high confidence on its own.
                    (
                        "intl",
                        r"(?:\+|00)[1-9]\d{0,2}[ .\-]?\(?\d{1,4}\)?(?:[ .\-]?\d{2,4}){2,4}",
                        0.85,
                    ),
                    // Spanish 9-digit numbers: need nearby context to count.
                    (
                        "es_national",
                        r"\b[6789]\d{2}[ .\-]?\d{3}[ .\-]?\d{3}\b",
                        0.35,
                    ),
                ],
                vec![
                    "tel",
                    "tél",
                    "phone",
                    "teléfono",
                    "telefono",
                    "telèfon",
                    "mòbil",
                    "móvil",
                    "movil",
                    "mobile",
                    "fax",
                    "whatsapp",
                    "llamar",
                    "trucar",
                    "call",
                    "contact",
                    "contacto",
                    "contacte",
                ],
                0.35,
                vec![],
            )),
            Box::new(regex_recognizer(
                "iban",
                "IBAN_CODE",
                vec![("iban", r"\b[A-Z]{2}\d{2}(?:[ ]?[A-Z0-9]){10,30}\b", 0.9)],
                vec![],
                0.0,
                vec!["iban"],
            )),
            Box::new(regex_recognizer(
                "credit_card",
                "CREDIT_CARD",
                vec![("card", r"\b(?:\d[ \-]?){12,18}\d\b", 0.75)],
                vec![],
                0.0,
                vec!["luhn"],
            )),
            Box::new(regex_recognizer(
                "ip",
                "IP_ADDRESS",
                vec![(
                    "ipv4",
                    r"\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\b",
                    0.6,
                )],
                vec![],
                0.0,
                vec![],
            )),
            Box::new(SpanishIdRecognizer::new()),
        ];
        Self {
            analyzer: Analyzer::new(recognizers),
        }
    }

    /// All recognized entities in `text`.
    pub fn scan(&self, text: &str) -> Vec<RecognizerResult> {
        self.analyzer.analyze(text, &[], SCORE_THRESHOLD).entities
    }

    /// Category counts for the Privacy Lens. Counts occurrences, and never
    /// keeps the matched values.
    pub fn summarize(&self, text: &str) -> PiiSummary {
        let mut summary = PiiSummary::default();
        for entity in self.scan(text) {
            if let Some(category) = category_of(entity.entity_type.as_str()) {
                *summary.categories.entry(category.to_string()).or_insert(0) += 1;
                summary.total += 1;
            }
        }
        summary
    }

    /// "Copy without personal information": replace recognized identifiers
    /// with typed placeholders, locally.
    pub fn redact(&self, text: &str) -> String {
        let entities = self.scan(text);
        if entities.is_empty() {
            return text.to_string();
        }
        let mut operators: HashMap<String, Operator> = HashMap::new();
        for e in &entities {
            let key = e.entity_type.as_str().to_string();
            operators.entry(key.clone()).or_insert(Operator::Replace {
                new_value: placeholder_of(&key).to_string(),
            });
        }
        let default_op = Operator::Replace {
            new_value: "[PERSONAL_INFORMATION]".to_string(),
        };
        Anonymizer::anonymize(text, &entities, &operators, &default_op, None).text
    }

    /// Whether `text` contains any recognized personal information — used to
    /// decide whether to offer "Copy without personal information".
    pub fn contains_pii(&self, text: &str) -> bool {
        !self.scan(text).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner() -> PiiScanner {
        PiiScanner::new()
    }

    #[test]
    fn detects_spec_example() {
        let text = "Please pay ES7620770024003102575766 and email maria@example.com.";
        let summary = scanner().summarize(text);
        assert_eq!(summary.categories.get("iban"), Some(&1));
        assert_eq!(summary.categories.get("email"), Some(&1));
    }

    #[test]
    fn redacts_like_the_spec() {
        let text = "Please pay ES7620770024003102575766 and email maria@example.com.";
        let redacted = scanner().redact(text);
        assert_eq!(redacted, "Please pay [IBAN] and email [EMAIL_ADDRESS].");
    }

    #[test]
    fn validates_nif_checksums() {
        // 12345678Z is the canonical valid example (12345678 % 23 = 14 → 'Z').
        assert!(nif_valid("12345678Z"));
        assert!(!nif_valid("12345678A"));
        let s = scanner();
        let found = s.summarize("Employee NIF: 12345678Z");
        assert_eq!(found.categories.get("nif"), Some(&1));
        let none = s.summarize("Employee NIF: 12345678A");
        assert_eq!(none.categories.get("nif"), None);
    }

    #[test]
    fn validates_nie_checksums() {
        // X1234567L: 01234567 % 23 = 12 → 'L'.
        assert!(nie_valid("X1234567L"));
        assert!(!nie_valid("X1234567T"));
        let found = scanner().summarize("NIE X1234567L");
        assert_eq!(found.categories.get("nie"), Some(&1));
    }

    #[test]
    fn credit_card_needs_luhn() {
        let s = scanner();
        let ok = s.summarize("Card: 4111 1111 1111 1111");
        assert_eq!(ok.categories.get("credit_card"), Some(&1));
        let bad = s.summarize("Card: 4111 1111 1111 1112");
        assert_eq!(bad.categories.get("credit_card"), None);
    }

    #[test]
    fn phones_need_prefix_or_context() {
        let s = scanner();
        // Bare 9 digits that could be an invoice number: not counted.
        let bare = s.summarize("Reference 612345678 approved");
        assert_eq!(bare.categories.get("phone"), None);
        // Same digits next to a phone context word: counted.
        let ctx = s.summarize("Teléfono: 612 345 678");
        assert_eq!(ctx.categories.get("phone"), Some(&1));
        // International prefix alone is enough.
        let intl = s.summarize("Reach us at +34 612 345 678");
        assert_eq!(intl.categories.get("phone"), Some(&1));
    }

    #[test]
    fn no_pii_no_changes() {
        let s = scanner();
        let text =
            "EBITDA stands for earnings before interest, taxes, depreciation and amortization.";
        assert!(!s.contains_pii(text));
        assert_eq!(s.redact(text), text);
    }
}
