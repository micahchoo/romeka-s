"""Write the two forms of the seam spike (crates/omeka-db/README.md, "Where the engine is chosen").

Usage: python3 gen.py N OUT. OUT gets a workspace with spike-generic and spike-enum, each N
handlers from one template; then run measure.sh inside OUT.
"""
import sys, os
N = int(sys.argv[1]) if len(sys.argv) > 1 else 60
OUT = sys.argv[2] if len(sys.argv) > 2 else "."
DB_CRATE = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
REPOS = [
 ("AssetRepo", "list(&AssetFilter::default(), &Paging::default())", "name"),
 ("JobRepo", "list(&JobFilter::default(), &Paging::default())", "class"),
 ("ModuleRepo", "list()", "version"),
 ("UserRepo", "list(&UserFilter::default(), &Paging::default())", "email"),
 ("SiteRepo", "list_all_pages()", "slug"),
 ("ResourceRepo", "list_of_type(\"Omeka\\\\Entity\\\\Item\")", "resource_type"),
 ("VocabularyRepo", "list(&VocabularyFilter::default(), &Paging::default())", "prefix"),
]
BOUNDS = """    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
"""
USES = """use omeka_db::Backend;
use omeka_db::repos::*;
use serde_json::json;
"""
def body(i, call):
    repo, meth, field = REPOS[i % len(REPOS)]
    return f"""    let rows = {call(repo, meth)}.map_err(|e| e.to_string())?;
    let mut docs: Vec<serde_json::Value> = rows
        .iter()
        .enumerate()
        .filter(|(n, _)| n % {i % 5 + 2} != 0)
        .map(|(n, r)| json!({{"n": n, "k": format!("{{}}-{i}", r.{field}), "len": r.{field}.len() * {i + 1}}}))
        .collect();
    docs.sort_by(|a, b| a["k"].as_str().cmp(&b["k"].as_str()).then(a["n"].as_u64().cmp(&b["n"].as_u64())));
    let total = docs.len();
    let page: Vec<_> = docs.into_iter().skip({i % 3}).take(25).collect();
    let body = serde_json::to_string(&json!({{"total": total, "page": page, "route": "r{i}"}})).map_err(|e| e.to_string())?;
    Ok(body)
"""
def cargo(name, extra=""):
    return f"""[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
omeka-db = {{ path = "{DB_CRATE}", features = ["mysql"] }}
serde_json = "1"
sqlx = {{ version = "0.8.6", default-features = false, features = ["runtime-tokio", "sqlite", "mysql"] }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
"""
def write(name, src):
    os.makedirs(f"{OUT}/{name}/src", exist_ok=True)
    open(f"{OUT}/{name}/Cargo.toml","w").write(cargo(name))
    open(f"{OUT}/{name}/src/main.rs","w").write(src)

# generic
g = USES + "use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};\n\n"
for i in range(N):
    g += f"pub async fn h{i}<DB>(pool: &Pool<DB>) -> Result<String, String>\nwhere\n{BOUNDS}{{\n" + body(i, lambda r, m: f"{r}::<DB>::new(pool).{m}.await") + "}\n\n"
g += f"pub async fn run_all<DB>(pool: &Pool<DB>) -> usize\nwhere\n{BOUNDS}{{\n    let mut n = 0;\n"
for i in range(N):
    g += f"    n += h{i}(pool).await.map(|s| s.len()).unwrap_or(0);\n"
g += "    n\n}\n\n"
g += """#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).unwrap_or_default();
    let n = if url.starts_with("mysql:") {
        let pool = omeka_db::open_mysql(&url).await.unwrap();
        run_all(&pool).await
    } else {
        let pool = omeka_db::open_memory().await.unwrap();
        omeka_db::migrate(&pool).await.unwrap();
        run_all(&pool).await
    };
    println!("{n}");
}
"""
write("spike-generic", g)

e = USES + """use sqlx::{MySqlPool, SqlitePool};

pub enum DbPool {
    Sqlite(SqlitePool),
    MySql(MySqlPool),
}

macro_rules! with_db {
    ($pool:expr, $p:ident => $e:expr) => {
        match $pool {
            DbPool::Sqlite($p) => $e,
            DbPool::MySql($p) => $e,
        }
    };
}

"""
for i in range(N):
    e += f"pub async fn h{i}(pool: &DbPool) -> Result<String, String> {{\n" + body(i, lambda r, m: f"with_db!(pool, p => {r}::new(p).{m}.await)") + "}\n\n"
e += "pub async fn run_all(pool: &DbPool) -> usize {\n    let mut n = 0;\n"
for i in range(N):
    e += f"    n += h{i}(pool).await.map(|s| s.len()).unwrap_or(0);\n"
e += "    n\n}\n\n"
e += """#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).unwrap_or_default();
    let pool = if url.starts_with("mysql:") {
        DbPool::MySql(omeka_db::open_mysql(&url).await.unwrap())
    } else {
        let pool = omeka_db::open_memory().await.unwrap();
        omeka_db::migrate(&pool).await.unwrap();
        DbPool::Sqlite(pool)
    };
    println!("{}", run_all(&pool).await);
}
"""
write("spike-enum", e)
open(f"{OUT}/Cargo.toml","w").write('[workspace]\nresolver = "3"\nmembers = ["spike-generic", "spike-enum"]\n\n[profile.release]\nstrip = true\n')
