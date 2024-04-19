use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::process;
use time::format_description::well_known;
use time::OffsetDateTime;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROGRAM: &str = env!("CARGO_PKG_NAME");

fn now() -> String {
    let current_time = OffsetDateTime::now_utc();
    current_time.format(&well_known::Rfc3339).unwrap()
}

#[derive(Serialize, Deserialize, Debug)]
struct Author {
    name: Option<String>,
    url: Option<String>,
    avatar: Option<String>,
}

impl Author {
    fn into_output(self) -> String {
        let mut output = "<author>\n<name>".to_string();
        output += &self.name.unwrap_or_default();
        output += "</name>\n";
        if let Some(url) = self.url {
            output += &format!("<uri>{}</uri>\n", url);
        }
        output += "</author>\n";
        output
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Hub {
    r#type: String,
    url: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Item {
    id: String,
    url: Option<String>,
    external_url: Option<String>,
    title: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    summary: Option<String>,
    image: Option<String>,
    banner_image: Option<String>,
    date_published: Option<String>,
    date_modified: Option<String>,
    authors: Option<Vec<Author>>,
    author: Option<Author>, // for compatibility with JSON Feed 1.0
    tags: Option<Vec<String>>,
    language: Option<String>,
    attachments: Option<Vec<Attachment>>,
}

impl Item {
    fn into_output(mut self) -> String {
        let mut output = "".to_string();

        if let Some(language) = &self.language {
            output += &format!("<entry xml:lang=\"{}\">\n", language);
        } else {
            output += "<entry>\n";
        };

        output += &format!("<id>{}</id>\n", &self.id);
        if let Some(title) = &self.title {
            output += &format!("<title>{}</title>\n", &title);
        }

        if let Some(url) = &self.url {
            output += &format!("<link rel=\"alternate\" href=\"{}\"/>\n", &url);
        }

        if let Some(summary) = &self.summary {
            output += &format!("<summary>{}</summary>\n", &summary);
        }

        if let Some(content_text) = &self.content_text {
            output += &format!("<content type=\"text\">{}</content>\n", &content_text);
        } else if let Some(content_html) = &self.content_html {
            output += &format!(
                "<content type=\"html\"><![CDATA[ {} ]]></content>\n",
                &content_html
            );
        }

        let updated = if let Some(date_modified) = &self.date_modified {
            date_modified.to_string()
        } else if let Some(date_published) = &self.date_published {
            date_published.to_string()
        } else {
            now()
        };

        output += &format!("<updated>{}</updated>\n", updated);

        if let Some(date_published) = &self.date_published {
            output += &format!("<published>{}</published>\n", &date_published);
        }

        if self.author.is_some() {
            if self.authors.is_none() {
                self.authors = Some(vec![self.author.unwrap()]);
            }

            self.author = None;
        }

        if let Some(authors) = self.authors {
            for author in authors {
                output += &author.into_output();
            }
        }

        if let Some(attachments) = self.attachments {
            for attachment in attachments {
                output += &format!(
                    "<link rel=\"enclosure\" href=\"{}\"/ type=\"{}\"",
                    &attachment.url, &attachment.mime_type
                );

                if let Some(size_in_bytes) = &attachment.size_in_bytes {
                    output += &format!(" length=\"{}\"", &size_in_bytes);
                }

                output += ">\n";
            }
        }

        output += "</entry>\n";
        output
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Attachment {
    url: String,
    mime_type: String,
    title: Option<String>,
    size_in_bytes: Option<u64>,
    duration_in_seconds: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Feed {
    version: String,
    title: String,
    home_page_url: Option<String>,
    feed_url: Option<String>,
    description: Option<String>,
    user_comment: Option<String>,
    next_url: Option<String>,
    icon: Option<String>,
    favicon: Option<String>,
    authors: Option<Vec<Author>>,
    author: Option<Author>, // for compatibility with JSON Feed 1.0
    language: Option<String>,
    expired: Option<bool>,
    hubs: Option<Vec<Hub>>, // TODO: Can this be used in output?
    items: Option<Vec<Item>>,
}

impl Feed {
    fn into_output(mut self) -> String {
        let mut output = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n".to_string();

        if let Some(language) = &self.language {
            output += &format!(
                "<feed xmlns=\"http://www.w3.org/2005/Atom\" xml:lang=\"{}\">\n",
                language
            );
        } else {
            output += "<feed xmlns=\"http://www.w3.org/2005/Atom\">\n";
        };

        if self.author.is_some() {
            if self.authors.is_none() {
                self.authors = Some(vec![self.author.unwrap()]);
            }

            self.author = None;
        }

        let mut author_exists = false;
        if let Some(authors) = self.authors {
            for author in authors {
                output += &author.into_output();
                author_exists = true;
            }
        }

        if !author_exists {
            output += "<author><name></name></author>\n";
        }

        output += &format!("<title>{}</title>\n", self.title);

        if let Some(feed_url) = &self.feed_url {
            output += &format!("<id>{}</id>\n", &feed_url);
        } else {
            output += &format!("<id>{}</id>\n", &self.title);
        }

        if let Some(home_page_url) = &self.home_page_url {
            output += &format!("<link rel=\"alternate\" href=\"{}\"/>\n", home_page_url);
        }

        if let Some(feed_url) = &self.feed_url {
            output += &format!("<link rel=\"self\" href=\"{}\"/>\n", feed_url);
        }

        if let Some(description) = &self.description {
            output += &format!("<subtitle>{}</subtitle>\n", description);
        }

        if let Some(icon) = &self.icon {
            output += &format!("<logo>{}</logo>\n", icon);
        }

        let mut updated = "2000-01-01T00:00:00Z".to_string();
        if let Some(items) = &self.items {
            for item in items {
                if let Some(item_updated) = &item.date_modified {
                    if updated.cmp(item_updated) == Ordering::Less {
                        updated = item_updated.to_string();
                    }
                } else if let Some(item_published) = &item.date_published {
                    if updated.cmp(item_published) == Ordering::Less {
                        updated = item_published.to_string();
                    }
                }
            }
        }

        output += &format!("<updated>{}</updated>\n", updated);

        if let Some(items) = self.items {
            for item in items {
                output += &item.into_output();
            }
        }

        output += "</feed>";
        output
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut input = None;
    let mut output = None;

    if args.len() > 1 {
        if args[1] == "--help" || args[1] == "-h" {
            let mut help = format!("{} {}\n", PROGRAM, VERSION).to_string();
            help += "Converts a JSON Feed to Atom. ";
            help += "Learn about JSON Feed: https://jsonfeed.org/\n\n";
            help += &format!("Usage:\n    {} [[input] output]\n\n", PROGRAM);
            help += "input is a path to a JSON Feed file.\n";
            help += "output is a path to an Atom file (use - to write to stdout).";
            println!("{}", help);
            process::exit(0);
        } else if args[1] == "--version" {
            println!("{} {}", PROGRAM, VERSION);
            process::exit(0);
        } else {
            output = Some(args[1].to_string());
        }
    } else if args.len() > 2 {
        input = Some(args[1].to_string());
        output = Some(args[2].to_string());
    }

    if let Some(ref d) = output {
        if d == "-" {
            output = None;
        }
    }

    let data = if let Some(input) = input {
        fs::read_to_string(input).unwrap()
    } else {
        eprintln!("Reading from stdin...");
        let lines = io::stdin().lock().lines();
        let mut stdin_data = String::new();

        for line in lines.map_while(Result::ok) {
            if line.is_empty() {
                break;
            }

            if !stdin_data.is_empty() {
                stdin_data.push('\n');
            }

            stdin_data.push_str(&line);
        }

        stdin_data
    };

    let feed: Feed = serde_json::from_str(&data).unwrap();
    let feed_atom = feed.into_output();

    if let Some(output) = output {
        // TODO: Check whether there is new content (otherwise don't write file)
        let mut output = File::create(output).unwrap();
        writeln!(output, "{}", feed_atom).unwrap();
    } else {
        println!("{}", feed_atom);
    }
}
