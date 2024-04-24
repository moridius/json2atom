use jfeed::{Author, Feed, Item};
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::process;
use time::format_description::well_known;
use time::{OffsetDateTime, UtcOffset};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROGRAM: &str = env!("CARGO_PKG_NAME");

fn now() -> String {
    let current_time = OffsetDateTime::now_utc();
    current_time.format(&well_known::Rfc3339).unwrap()
}

fn get_mtime(file: &str) -> Option<OffsetDateTime> {
    if let Ok(metadata) = fs::metadata(file) {
        if let Ok(modified) = metadata.modified() {
            let mut odt: OffsetDateTime = modified.into();
            if let Ok(offset) = UtcOffset::local_offset_at(odt) {
                odt = odt.to_offset(offset);
                return Some(odt);
            }
        }
    }

    None
}

trait ToAtom {
    fn to_atom(&self) -> String;

    fn updated(&self) -> Option<OffsetDateTime> {
        None
    }
}

impl ToAtom for Author {
    fn to_atom(&self) -> String {
        let mut output = "<author>\n<name>".to_string();
        if let Some(name) = &self.name {
            output += &name;
        }
        output += "</name>\n";

        if let Some(url) = &self.url {
            output += &format!("<uri>{}</uri>\n", url);
        }
        output += "</author>\n";

        output
    }
}

impl ToAtom for Item {
    fn updated(&self) -> Option<OffsetDateTime> {
        if let Some(date_modified) = &self.date_modified {
            return OffsetDateTime::parse(date_modified, &well_known::Rfc3339).ok();
        }

        if let Some(date_published) = &self.date_published {
            return OffsetDateTime::parse(date_published, &well_known::Rfc3339).ok();
        };

        None
    }

    fn to_atom(&self) -> String {
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

        if let Some(authors) = &self.authors {
            for author in authors {
                output += &author.to_atom();
            }
        }

        if let Some(attachments) = &self.attachments {
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

impl ToAtom for Feed {
    fn updated(&self) -> Option<OffsetDateTime> {
        let mut updated = None;

        if let Some(items) = &self.items {
            for item in items {
                if let Some(item_updated) = item.updated() {
                    if let Some(u) = updated {
                        if item_updated > u {
                            updated = Some(item_updated);
                        }
                    } else {
                        updated = Some(item_updated);
                    }
                }
            }
        }

        updated
    }

    fn to_atom(&self) -> String {
        let mut output = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n".to_string();

        if let Some(language) = &self.language {
            output += &format!(
                "<feed xmlns=\"http://www.w3.org/2005/Atom\" xml:lang=\"{}\">\n",
                language
            );
        } else {
            output += "<feed xmlns=\"http://www.w3.org/2005/Atom\">\n";
        };

        let mut author_exists = false;
        if let Some(authors) = &self.authors {
            for author in authors {
                output += &author.to_atom();
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

        if let Some(updated) = self.updated() {
            output += &format!("<updated>{}</updated>\n", updated);
        } else {
            output += &format!("<updated>{}</updated>\n", now());
        }

        if let Some(items) = &self.items {
            for item in items {
                output += &item.to_atom();
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
            help += "output is a path to an Atom file (use - to write to stdout).\n\n";
            help += "-h, --help     show this help and exit\n";
            help += "    --version  show version information and exit\n";
            help +=
                "-f, --force    rewrite file even if modification time is newer than the feed\n";
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

    if let Ok(feed) = Feed::parse(&data) {
        let feed_atom = feed.to_atom();

        let updated = if let Some(updated) = feed.updated() {
            updated
        } else {
            OffsetDateTime::now_utc()
        };

        if let Some(output) = output {
            let write_file = if let Some(mtime) = get_mtime(&output) {
                updated > mtime
            } else {
                true
            };

            if write_file {
                let mut output = File::create(output).unwrap();
                writeln!(output, "{}", feed_atom).unwrap();
            }
        } else {
            println!("{}", feed_atom);
        }
    } else {
        eprintln!("Cannot parse feed.");
        process::exit(1);
    }
}
