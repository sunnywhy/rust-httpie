use std::{collections::HashMap, str::FromStr};

use anyhow::{anyhow, Result};
use clap::Parser;
use colored::*;
use mime::Mime;
use reqwest::{header, Client, Response, Url};
use syntect::{parsing::SyntaxSet, highlighting::ThemeSet, easy::HighlightLines, util::{LinesWithEndings, as_24_bit_terminal_escaped}};

#[derive(Parser, Debug)]
#[clap(version = "1.0", author = "Wei L. <sunnywhy@gmail.com>")]
struct Opts {
    #[clap(subcommand)]
    subcmd: Subcommand,
}

#[derive(Parser, Debug)]
enum Subcommand {
    Get(Get),
    Post(Post),
}

#[derive(Parser, Debug)]
struct Get {
    #[clap(parse(try_from_str = parse_url))]
    url: String,
}

#[derive(Parser, Debug)]
struct Post {
    #[clap(parse(try_from_str = parse_url))]
    url: String,
    body: Vec<KvPair>,
}

fn parse_url(url: &str) -> Result<String> {
    let url: Url = url.parse()?;

    Ok(url.into())
}

#[derive(Debug, PartialEq, Eq)]
struct KvPair {
    k: String,
    v: String,
}

impl FromStr for KvPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split('=');
        let err = || anyhow!(format!("Failed to parse {}", s));
        Ok(Self {
            k: (split.next().ok_or_else(err)?).to_string(),
            v: (split.next().ok_or_else(err)?).to_string(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    // add default HTTP headers
    let mut headers = header::HeaderMap::new();
    headers.insert("X-POWERED-BY", "Rust".parse()?);
    headers.insert(header::USER_AGENT, "Rust Httpie".parse()?);

    let client = Client::builder().default_headers(headers).build()?;
    let result = match opts.subcmd {
        Subcommand::Get(ref args) => get(client, args).await?,
        Subcommand::Post(ref args) => post(client, args).await?,
    };

    Ok(result)
}

async fn post(client: Client, args: &Post) -> Result<()> {
    let mut body = HashMap::new();
    for kv_pair in args.body.iter() {
        body.insert(&kv_pair.k, &kv_pair.v);
    }

    let response = client.post(&args.url).json(&body).send().await?;

    Ok(print_response(response).await?)
}

async fn get(client: Client, args: &Get) -> Result<()> {
    let response = client.get(&args.url).send().await?;
    Ok(print_response(response).await?)
}

async fn print_response(response: Response) -> Result<()> {
    print_status(&response);
    print_headers(&response);
    let mime = get_content_type(&response);
    let body = response.text().await?;
    print_body(mime, &body);
    Ok(())
}

fn print_body(mime: Option<Mime>, body: &str) {
    match mime {
        Some(v) if v == mime::APPLICATION_JSON => print_syntect(body, "json"),
        Some(v) if v == mime::TEXT_HTML => print_syntect(body, "html"),
        _ => println!("{}", body),
    }
}

fn print_syntect(body: &str, ext: &str) {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];
    let syntax = ss.find_syntax_by_extension(ext).unwrap();
    let mut h = HighlightLines::new(syntax, theme);
    for line in LinesWithEndings::from(body) {
        let ranges = h.highlight(line, &ss);
        let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
        println!("{}", escaped);
    }
}

fn get_content_type(response: &Response) -> Option<Mime> {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().parse().unwrap())
}

fn print_headers(response: &Response) {
    for (name, value) in response.headers() {
        println!("{}: {:?}", name.to_string().green(), value);
    }
    println!();
}

fn print_status(response: &Response) {
    let status = format!("{:?} {}", response.version(), response.status()).blue();
    println!("{}\n", status);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_works() {
        assert!(parse_url("url").is_err());
        assert!(parse_url("http://abc.xyz").is_ok());
        assert!(parse_url("https://httpbin.org/post").is_ok());
    }

    #[test]
    fn parse_kv_pair_works() {
        assert!(KvPair::from_str("a").is_err());
        assert_eq!(
            KvPair::from_str("a=1").unwrap(),
            KvPair {
                k: "a".into(),
                v: "1".into(),
            }
        );
        assert_eq!(
            KvPair::from_str("b=").unwrap(),
            KvPair {
                k: "b".into(),
                v: "".into(),
            }
        );
    }
}
