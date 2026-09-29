//! Retrieval-quality evaluation.
//!
//! A small labelled legal corpus, built around the failure contextual headers
//! target: look-alike clauses from different cases (two leases, two
//! settlements) whose text never names the party. Questions name the case;
//! only the document's context can tell the clauses apart. Generic questions
//! check nothing regresses.
//!
//! It scores the embedding recipe alone — vector ranking, recall@3 and MRR —
//! for four variants: plain text, task prefixes, contextual headers, both.
//!
//!   cargo test --lib eval -- --nocapture                    # deterministic check
//!   bun run eval:retrieval                                  # live, against Ollama
//!
//! The live run uses the default provider (Ollama at localhost with
//! `nomic-embed-text`), or `SATO_EVAL_URL` / `SATO_EVAL_MODEL` if set.
//! `SATO_EVAL_VERBOSE=1` prints every question's rank per recipe.
//!
//! Result that set the production recipe (nomic-embed-text, 19 passages,
//! 18 questions): MRR plain 0.843, prefix 0.787, header 0.972,
//! prefix+header 0.944; recall@3 1.00 throughout. Headers fixed every
//! look-alike confusion between cases; prefixes never helped a question.

#![cfg(test)]

use crate::embedtext::{context_header, task_prefix, Role};

pub struct Passage {
    pub id: &'static str,
    pub case_ref: &'static str,
    pub file: &'static str,
    pub page: i64,
    pub text: &'static str,
}

pub struct Query {
    pub text: &'static str,
    pub relevant: &'static str,
    /// Answerable only with the document's context (case, file).
    pub needs_context: bool,
}

pub const CORPUS: &[Passage] = &[
    Passage { id: "hen-lease-1", case_ref: "henderson-v-ardent", file: "henderson-lease.pdf", page: 1, text: "The tenant shall give sixty days written notice before terminating this lease." },
    Passage { id: "hen-lease-2", case_ref: "henderson-v-ardent", file: "henderson-lease.pdf", page: 2, text: "Rent of 2,400 per month is payable on the first business day of each month." },
    Passage { id: "hen-settle-1", case_ref: "henderson-v-ardent", file: "henderson-settlement.pdf", page: 1, text: "The defendant shall pay the sum of 85,000 within thirty days of this agreement." },
    Passage { id: "oka-lease-1", case_ref: "okafor-v-brightwell", file: "okafor-lease.pdf", page: 1, text: "The tenant shall give ninety days written notice before terminating this lease." },
    Passage { id: "oka-lease-2", case_ref: "okafor-v-brightwell", file: "okafor-lease.pdf", page: 2, text: "Rent of 3,100 per month is payable on the fifth day of each month." },
    Passage { id: "oka-settle-1", case_ref: "okafor-v-brightwell", file: "okafor-settlement.pdf", page: 1, text: "The defendant shall pay the sum of 40,000 in four equal instalments." },
    Passage { id: "men-contract-1", case_ref: "mendes-employment", file: "mendes-contract.docx", page: 1, text: "The employee is entitled to twenty five days of paid annual leave." },
    Passage { id: "men-contract-2", case_ref: "mendes-employment", file: "mendes-contract.docx", page: 2, text: "Either party may terminate employment with one month notice in writing." },
    Passage { id: "kow-indict-1", case_ref: "state-v-kowalski", file: "kowalski-indictment.pdf", page: 1, text: "The accused is charged with two counts of fraud under section 7." },
    Passage { id: "kow-indict-2", case_ref: "state-v-kowalski", file: "kowalski-indictment.pdf", page: 2, text: "The offences are alleged to have occurred between March and June." },
    // More look-alikes and distractors, so rankings have room to differ.
    Passage { id: "hen-lease-3", case_ref: "henderson-v-ardent", file: "henderson-lease.pdf", page: 3, text: "The landlord is responsible for structural repairs to the building." },
    Passage { id: "oka-lease-3", case_ref: "okafor-v-brightwell", file: "okafor-lease.pdf", page: 3, text: "The landlord must repair the roof and exterior walls within fourteen days of a request." },
    Passage { id: "hen-settle-2", case_ref: "henderson-v-ardent", file: "henderson-settlement.pdf", page: 2, text: "Payment discharges all claims arising from the tenancy, known or unknown." },
    Passage { id: "oka-settle-2", case_ref: "okafor-v-brightwell", file: "okafor-settlement.pdf", page: 2, text: "Each party bears its own legal costs." },
    Passage { id: "men-contract-3", case_ref: "mendes-employment", file: "mendes-contract.docx", page: 3, text: "Salary is reviewed annually each April by the remuneration committee." },
    Passage { id: "men-contract-4", case_ref: "mendes-employment", file: "mendes-contract.docx", page: 4, text: "For six months after leaving, the employee may not work for a business that competes with the company." },
    Passage { id: "kow-indict-3", case_ref: "state-v-kowalski", file: "kowalski-indictment.pdf", page: 3, text: "The prosecution relies on bank records and emails exchanged between the defendants." },
    Passage { id: "ruiz-complaint-1", case_ref: "ruiz-v-city", file: "ruiz-complaint.pdf", page: 1, text: "The plaintiff slipped on an unmarked wet floor at the municipal library and fractured her wrist." },
    Passage { id: "ruiz-complaint-2", case_ref: "ruiz-v-city", file: "ruiz-complaint.pdf", page: 2, text: "Medical expenses of 12,000 are claimed together with four months of lost wages." },
];

pub const QUERIES: &[Query] = &[
    Query {
        text: "How much notice must the tenant give in the Henderson lease?",
        relevant: "hen-lease-1",
        needs_context: true,
    },
    Query {
        text: "What notice period applies to the Okafor tenancy?",
        relevant: "oka-lease-1",
        needs_context: true,
    },
    Query {
        text: "How much does the defendant pay under the Henderson settlement?",
        relevant: "hen-settle-1",
        needs_context: true,
    },
    Query {
        text: "What are the payment terms of the Okafor settlement?",
        relevant: "oka-settle-1",
        needs_context: true,
    },
    Query {
        text: "When is rent due for Okafor?",
        relevant: "oka-lease-2",
        needs_context: true,
    },
    Query {
        text: "What rent is due under the Henderson lease?",
        relevant: "hen-lease-2",
        needs_context: true,
    },
    Query {
        text: "How many leave days does Mendes get?",
        relevant: "men-contract-1",
        needs_context: false,
    },
    Query {
        text: "What charges does Kowalski face?",
        relevant: "kow-indict-1",
        needs_context: false,
    },
    Query {
        text: "How can the employment be ended?",
        relevant: "men-contract-2",
        needs_context: false,
    },
    Query {
        text: "When did the alleged offences take place?",
        relevant: "kow-indict-2",
        needs_context: false,
    },
    Query {
        text: "Who pays for fixing the building structure in Henderson?",
        relevant: "hen-lease-3",
        needs_context: true,
    },
    Query {
        text: "Who covers legal fees in the Okafor settlement?",
        relevant: "oka-settle-2",
        needs_context: true,
    },
    // Paraphrases: little or no wording shared with the answer. This is where
    // query/passage asymmetry, and so the task prefixes, should matter.
    Query {
        text: "Is there a non-compete restriction on the worker?",
        relevant: "men-contract-4",
        needs_context: false,
    },
    Query {
        text: "What proof does the state have against the accused?",
        relevant: "kow-indict-3",
        needs_context: false,
    },
    Query {
        text: "How did the accident happen?",
        relevant: "ruiz-complaint-1",
        needs_context: false,
    },
    Query {
        text: "What compensation is sought for the injury?",
        relevant: "ruiz-complaint-2",
        needs_context: false,
    },
    Query {
        text: "Does the settlement release every other claim?",
        relevant: "hen-settle-2",
        needs_context: false,
    },
    Query {
        text: "When does the employee get a pay rise?",
        relevant: "men-contract-3",
        needs_context: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recipe {
    pub prefix: bool,
    pub header: bool,
}

pub const RECIPES: [(&str, Recipe); 4] = [
    (
        "plain",
        Recipe {
            prefix: false,
            header: false,
        },
    ),
    (
        "prefix",
        Recipe {
            prefix: true,
            header: false,
        },
    ),
    (
        "header",
        Recipe {
            prefix: false,
            header: true,
        },
    ),
    (
        "prefix+header",
        Recipe {
            prefix: true,
            header: true,
        },
    ),
];

fn doc_text(model: &str, r: Recipe, p: &Passage) -> String {
    let prefix = if r.prefix {
        task_prefix(model, Role::Document)
    } else {
        ""
    };
    if r.header {
        format!(
            "{prefix}{}\n\n{}",
            context_header(Some(p.case_ref), p.file, Some(p.page)),
            p.text
        )
    } else {
        format!("{prefix}{}", p.text)
    }
}

fn query_text(model: &str, r: Recipe, q: &Query) -> String {
    let prefix = if r.prefix {
        task_prefix(model, Role::Query)
    } else {
        ""
    };
    format!("{prefix}{}", q.text)
}

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub recall_at_3: f64,
    pub mrr: f64,
    /// Recall@3 over the context-dependent questions only.
    pub context_recall_at_3: f64,
}

fn unit(v: &[f32]) -> Vec<f32> {
    let n = v
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt()
        .max(f32::EPSILON);
    v.iter().map(|x| x / n).collect()
}

/// Ranks the corpus for every question with `embed` and scores the ranking.
pub fn evaluate(model: &str, r: Recipe, embed: &dyn Fn(&[String]) -> Vec<Vec<f32>>) -> Metrics {
    let docs: Vec<Vec<f32>> = embed(
        &CORPUS
            .iter()
            .map(|p| doc_text(model, r, p))
            .collect::<Vec<_>>(),
    )
    .iter()
    .map(|v| unit(v))
    .collect();
    let queries: Vec<Vec<f32>> = embed(
        &QUERIES
            .iter()
            .map(|q| query_text(model, r, q))
            .collect::<Vec<_>>(),
    )
    .iter()
    .map(|v| unit(v))
    .collect();

    let (mut hits, mut rr, mut ctx_hits, mut ctx_n) = (0usize, 0f64, 0usize, 0usize);
    for (q, qv) in QUERIES.iter().zip(&queries) {
        let mut ranked: Vec<(usize, f32)> = docs
            .iter()
            .enumerate()
            .map(|(i, dv)| (i, dv.iter().zip(qv).map(|(a, b)| a * b).sum()))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let rank = ranked
            .iter()
            .position(|(i, _)| CORPUS[*i].id == q.relevant)
            .map(|p| p + 1);
        if std::env::var_os("SATO_EVAL_VERBOSE").is_some() {
            let top = CORPUS[ranked[0].0].id;
            println!(
                "  [{:<13}] rank {:<3} top={:<16} {}",
                format!(
                    "{}{}",
                    if r.prefix { "P" } else { "-" },
                    if r.header { "H" } else { "-" }
                ),
                rank.map_or("-".into(), |r| r.to_string()),
                top,
                q.text
            );
        }
        let top3 = rank.is_some_and(|r| r <= 3);
        hits += top3 as usize;
        rr += rank.map_or(0.0, |r| 1.0 / r as f64);
        if q.needs_context {
            ctx_n += 1;
            ctx_hits += top3 as usize;
        }
    }
    let n = QUERIES.len() as f64;
    Metrics {
        recall_at_3: hits as f64 / n,
        mrr: rr / n,
        context_recall_at_3: ctx_hits as f64 / ctx_n.max(1) as f64,
    }
}

fn table(model: &str, embed: &dyn Fn(&[String]) -> Vec<Vec<f32>>) -> Vec<(&'static str, Metrics)> {
    let rows: Vec<(&'static str, Metrics)> = RECIPES
        .iter()
        .map(|(name, r)| (*name, evaluate(model, *r, embed)))
        .collect();
    println!(
        "\nretrieval eval — model: {model} — {} passages, {} questions",
        CORPUS.len(),
        QUERIES.len()
    );
    println!(
        "{:<15} {:>10} {:>8} {:>22}",
        "recipe", "recall@3", "MRR", "context-q recall@3"
    );
    for (name, m) in &rows {
        println!(
            "{:<15} {:>10.2} {:>8.3} {:>22.2}",
            name, m.recall_at_3, m.mrr, m.context_recall_at_3
        );
    }
    rows
}

/// A deterministic stand-in for an embedding model: hashed bag of words. It
/// knows nothing of meaning, so it only checks the harness and the mechanism —
/// that the header puts case context where a question can match it.
fn bag_of_words(inputs: &[String]) -> Vec<Vec<f32>> {
    const DIM: usize = 512;
    inputs
        .iter()
        .map(|s| {
            let mut v = vec![0f32; DIM];
            for tok in s
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| t.len() > 2)
            {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for b in tok.bytes() {
                    h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
                }
                v[(h % DIM as u64) as usize] += 1.0;
            }
            v
        })
        .collect()
}

#[test]
fn eval_harness_measures_the_header_effect() {
    let rows = table("bag-of-words", &bag_of_words);
    let get = |n: &str| rows.iter().find(|(k, _)| *k == n).unwrap().1;
    let (plain, header) = (get("plain"), get("header"));
    // Without context, look-alike clauses from different cases are
    // indistinguishable; with it, the case named in the question decides.
    assert!(
        header.context_recall_at_3 > plain.context_recall_at_3,
        "{plain:?} vs {header:?}"
    );
    assert!(header.mrr >= plain.mrr);
    assert_eq!(header.context_recall_at_3, 1.0);
}

/// The live comparison. Needs a running embedding provider, so it is ignored
/// by default: `bun run eval:retrieval`.
#[test]
#[ignore]
fn live_retrieval_eval() {
    use crate::providers::{Provider, ProviderConfig};
    let mut cfg = ProviderConfig::default();
    if let Ok(url) = std::env::var("SATO_EVAL_URL") {
        cfg.base_url = url;
    }
    if let Ok(model) = std::env::var("SATO_EVAL_MODEL") {
        cfg.embedding_model = model;
    }
    let provider = Provider::new(cfg.clone()).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let diag = rt.block_on(provider.diagnose());
    if matches!(
        diag.problem,
        Some(crate::providers::Problem::Unreachable | crate::providers::Problem::Timeout)
    ) {
        println!(
            "\nskipped: {} is not reachable at {} ({})",
            cfg.kind, cfg.base_url, diag.detail
        );
        return;
    }
    // Only the embedding model matters here; a missing chat model does not.
    if diag.missing_models.contains(&cfg.embedding_model) {
        println!("\nskipped: run `ollama pull {}` first", cfg.embedding_model);
        return;
    }
    let embed = |inputs: &[String]| {
        rt.block_on(provider.embed(inputs))
            .expect("embedding failed")
    };
    table(&cfg.embedding_model, &embed);
}
