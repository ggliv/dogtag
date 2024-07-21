# dogtag
Dogtag is a web scraper for [the University of Georgia's online course catalog](https://sis-ssb-prod.uga.edu/PROD/bwckschd.p_disp_dyn_sched).

This tool gets:
| What | Example |
| ---- | ------- |
| Subjects | Computer Science |
| Subject codes | CSCI |
| Course titles | Algorithms |
| Course codes | 4470 |
| Instructors | Shelby H. Funk |
| Descriptions | Algorithms, covering basic analysis techniques, basic design techniques (divide-and-conquer, dynamic programming, greedy), basic and advanced graph algorithms, and NP-completeness theory. |
| Credit hours | 4.0 |
| CRNs | 31873 |
| Schedules | Mondays from 16:10 to 17:00 in Room 404A of Cedar Street Building B |

## Configuration
The config environment variable prefix for this program is `DT_`. For example, if you wanted to set the key "foo" to the value "bar", you would run the program with `DT_FOO=bar` in the environment.

Here are the values you can configure:
| Key | Description |
| --- | ----------- |
| term | The code of the term to scrape. Should be one of the option values listed on [this page](https://sis-ssb-prod.uga.edu/PROD/bwckschd.p_disp_dyn_sched) (use inspect element). |
| per_min_ratelimit | Maximum number of requests the scraper is allowed to make in 60 seconds. Set to a value <= 0 for no limit. |
| previous_scrape | Path to a file containing a prior scrape. Will be used for course descriptions and credit hours instead of making requests.[^0] |
| bulletin_home_url | You probably don't want to mess with this. |
| course_details_url | You probably don't want to mess with this. |
| course_search_url | You probably don't want to mess with this. |
| course_sched_url | You probably don't want to mess with this. |

In addition, this project uses [env_logger](https://docs.rs/env_logger/latest/env_logger/) to log status messages. To see all messages with a level of `INFO` or higher, run the program with `RUST_LOG=info` in the environment. See the env_logger docs for more information.

[^0]: You can use [this scrape of FA 2024 classes](https://files.catbox.moe/oc6fey.gz) that I did in July 2024 as a base if you want. Just keep in mind that some values may have changed since then.

## Running
Dogtag is a Rust project, so you'll need a local [Rust toolchain](https://rustup.rs/) to run it.

First, make sure you have your desired environment variables set up as described above. Then, run:
```sh
cargo run --release
```

## Schema
This program outputs JSON to stdout. Please refer to the structs in `src/structs.rs` to see how things are laid out.

As an example, if you wanted to see scraped information for Algorithms (CSCI 4470) and you had deserialized the output JSON into an object called `root`, you might do something like:
```
root["CSCI"].courses["4470"]
```

Remarks:
- Some values will be nulled/arrays will be empty if the course listing doesn't have that information. Be careful when consuming.
- Credit hours are represented as ranges (because [they can vary](https://sis-ssb-prod.uga.edu/PROD/bwckctlg.p_display_courses?term_in=202408&one_subj=POPH&sel_crse_strt=5410&sel_crse_end=5410&sel_subj=&sel_levl=&sel_schd=&sel_coll=&sel_divs=&sel_dept=&sel_attr=)). The first value is a minimum, the second is a maximum. If they're are the same, that's the single credit hour value.
- Days of the week are represented by their [ISO 8601 week day](https://en.wikipedia.org/wiki/ISO_8601#Week_dates) number. (i.e. Monday -> 1, Tuesday -> 2, ..., Sunday -> 7).

<!-- TODO maybe make this better lol -->

## TODO
- General code cleanup for readabilty/maintainability
- Save to file instead of just dumping to stdout
- Interactive mode (choose term/subjects to scrape at runtime)
- Pretty progress information
- Scrape registration availability info from "[Detailed Class Information](https://sis-ssb-prod.uga.edu/PROD/bwckschd.p_disp_detail_sched?term_in=202408&crn_in=31873)" pages
- Prerequisite chains?
