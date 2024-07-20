use crate::structs::*;

use crate::Result;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;

pub async fn go(config: config::Config, previous_scrape: Option<HashMap<String, Subject>>) -> Result<HashMap<String, Subject>> {
    let ctx = Context::new(config)?;
    let subjs = get_subject_titles(&ctx).await?;
    let sections_doc = get_sections_doc(&ctx, subjs.keys().map(|s| s.as_str())).await?;
    Ok(scrape_doc(&ctx, sections_doc, subjs, previous_scrape).await?)
}

struct Context {
    client: Client,
    governor: Option<DefaultDirectRateLimiter>,
    config: config::Config,
}

impl Context {
    fn new(config: config::Config) -> Result<Self> {
        let client = Client::builder().gzip(true).cookie_store(true).build()?;
        let governor = match config.get_int("per_min_ratelimit") {
            Ok(rl) if rl.is_positive() => Some(RateLimiter::direct(
                Quota::per_minute(NonZeroU32::new(rl.try_into()?).unwrap())
                    .allow_burst(NonZeroU32::new(1).unwrap()),
            )),
            _ => None,
        };

        Ok(Context {
            client,
            governor,
            config,
        })
    }

    async fn rate_limit(&self) {
        if let Some(gov) = &self.governor {
            log::info!("Waiting for rate limit...");
            gov.until_ready().await
        }
    }
}

async fn get_sections_doc(ctx: &Context, subjs: impl Iterator<Item = &str>) -> Result<Html> {
    let url = ctx.config.get_string("course_sched_url")?;
    let term = ctx.config.get_string("term")?;

    let subjs: Vec<(&str, &str)> = std::iter::repeat("sel_subj").zip(subjs).collect();

    ctx.rate_limit().await;

    log::info!("Requesting page with all sections (this will take a while)");

    let sections_page = ctx
        .client
        .post(url)
        .query(&[
            ("term_in", term.as_str()),
            ("sel_subj", "dummy"),
            ("sel_day", "dummy"),
            ("sel_schd", "dummy"),
            ("sel_insm", "dummy"),
            ("sel_camp", "dummy"),
            ("sel_levl", "dummy"),
            ("sel_sess", "dummy"),
            ("sel_instr", "dummy"),
            ("sel_ptrm", "dummy"),
            ("sel_attr", "dummy"),
        ])
        .query(subjs.as_slice())
        .query(&[
            ("sel_crse", ""),
            ("sel_title", ""),
            ("sel_schd", "%"),
            ("sel_from_cred", ""),
            ("sel_to_cred", ""),
            ("sel_camp", "%"),
            ("sel_levl", "%"),
            ("sel_ptrm", "%"),
            ("sel_instr", "%"),
            ("sel_attr", "%"),
            ("begin_hh", "0"),
            ("begin_mi", "0"),
            ("begin_ap", "a"),
            ("end_hh", "0"),
            ("end_mi", "0"),
            ("end_ap", "a"),
        ])
        .send()
        .await?
        .text()
        .await?;

    Ok(scraper::Html::parse_document(&sections_page))
}

async fn scrape_doc(
    ctx: &Context,
    doc: Html,
    subjs: HashMap<String, String>,
    previous_scrape: Option<HashMap<String, Subject>>,
) -> Result<HashMap<String, Subject>> {
    let mut map = HashMap::new();

    let class_sel = Selector::parse(
        "table[summary='This layout table is used to present the sections found'] > tbody > tr",
    )?;

    let mut classes = doc.select(&class_sel);

    while let (Some(line), Some(body)) = (classes.next(), classes.next()) {
        let (title, crn, subj, code) = match scrape_line(line) {
            Ok(r) => r,
            _ => {
                log::warn!("Could not parse info line `{}`, skipping", line.html());
                continue;
            }
        };

        // wow this is silly lmao
        let (schedule, instructors) = match scrape_body(body) {
            Ok(t) => t,
            _ => {
                log::warn!("Could not parse section body for CRN {crn}, skipping");
                continue;
            }
        };

        let section = Section {
            crn,
            instructors,
            schedule,
        };

        if !map.contains_key(&subj) {
            let subj_title = subjs.get(&subj);

            if subj_title.is_none() {
                log::warn!("Subject code {subj} has no available long title, nulling it")
            }

            map.insert(
                subj.clone(),
                Subject {
                    title: subjs.get(&subj).cloned(),
                    courses: HashMap::new(),
                },
            );
        }

        let courses = &mut map.get_mut(&subj).unwrap().courses;

        if !courses.contains_key(&code) {
            let cached = previous_scrape.as_ref().and_then(|m| {
                m.get(&subj).and_then(|s| {
                    s.courses
                        .get(&code)
                        .and_then(|c| Some((c.description.to_owned(), c.credits.to_owned())))
                })
            });

            let (description, credits) = match cached {
                Some(v) => v,
                None => get_course_catalog(ctx, &subj, &code).await?,
            };

            courses.insert(
                code.clone(),
                Course {
                    title: title.decode(),
                    description: description.decode(),
                    credits,
                    sections: Vec::new(),
                },
            );
        }

        courses.get_mut(&code).unwrap().sections.push(section);
        log::info!("Finished scraping a section of {subj} {code}");
    }

    if let Some(_) = classes.next() {
        log::warn!("Some section tables were not processed")
    }

    Ok(map)
}

async fn get_subject_titles(ctx: &Context) -> Result<HashMap<String, String>> {
    let mut titles = HashMap::new();
    let subj_re = Regex::new(r"([A-Z]{4})\s*[-–]\s*(.*)")?;

    let parse_subj_option = |e: ElementRef| -> Result<(String, String)> {
        let inner = e.inner_html();
        let groups = subj_re
            .captures(&inner)
            .ok_or_else(|| format!("Could not parse subject: {inner}"))?;
        Ok((groups[1].into(), groups[2].decode()))
    };

    ctx.rate_limit().await;

    // first, get titles from the course search page.
    // this should include all the subjects we'll encounter,
    // but some of the titles are shortened.
    let page = ctx
        .client
        .post(ctx.config.get_string("course_search_url")?)
        .query(&[
            ("p_calling_proc", "bwckschd.p_disp_dyn_sched"),
            ("p_term", &ctx.config.get_string("term")?),
        ])
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&page);
    let subjs_sel = Selector::parse("#subj_id > option")?;

    for subj in doc.select(&subjs_sel) {
        let (code, title) = parse_subj_option(subj)?;
        titles.insert(code, title);
    }

    ctx.rate_limit().await;

    // next, get titles from the bulletin. this
    // won't necessarily have all of the courses
    // we'll encounter, but the titles are full
    // (as in, not abbreviated)
    let page = ctx
        .client
        .get(ctx.config.get_string("bulletin_home_url")?)
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&page);
    let subjs_sel = Selector::parse("#ddlAllPrefixes > option")?;

    for subj in doc.select(&subjs_sel).skip(1) {
        let (code, title) = parse_subj_option(subj)?;
        if titles.contains_key(&code) {
            titles.insert(code, title);
        }
    }

    Ok(titles)
}

async fn get_course_catalog(ctx: &Context, subj: &str, code: &str) -> Result<(String, (f32, f32))> {
    ctx.rate_limit().await;

    let page = ctx
        .client
        .get(ctx.config.get_string("course_details_url")?)
        .query(&[
            ("cat_term_in", ctx.config.get_string("term")?.as_str()),
            ("subj_code_in", subj),
            ("crse_numb_in", code),
        ])
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&page);
    let body_sel = Selector::parse(
        "table[summary='This table lists the course detail for the selected term.'] td.ntdefault",
    )?;
    let body = doc.select(&body_sel).next().ok_or("body")?.inner_html();
    let br_re = Regex::new(r"<\s*br\s*\\?\s*>")?;
    let mut chunks = br_re.split(&body);
    let desc = chunks.next().ok_or("desc")?.trim();
    let credits = {
        let cred_line = chunks.next().ok_or("credits")?;
        let cred_range_re = Regex::new(r".*(\d+\.\d+)\s+TO\s+(\d+\.\d+) Credit hours.*")?;
        let cred_single_re = Regex::new(r".*(\d+\.\d+)\s+Credit hours.*")?;
        if let Some(caps) = cred_range_re.captures(cred_line) {
            (caps[1].parse::<f32>()?, caps[2].parse::<f32>()?)
        } else if let Some(caps) = cred_single_re.captures(cred_line) {
            (caps[1].parse::<f32>()?, caps[1].parse::<f32>()?)
        } else {
            Err("credits: no match")?
        }
    };

    Ok((desc.into(), credits))
}

fn scrape_line(line: ElementRef) -> Result<(String, usize, String, String)> {
    let line_re = Regex::new(r"(.*) - (\d{5}) - ([A-Z]{4}) (\d{4}[A-Z]?) - .*")?;
    let line_sel = Selector::parse(":scope > th > a")?;
    let line = line
        .select(&line_sel)
        .next()
        .ok_or("line find")?
        .inner_html();

    let caps = line_re.captures(&line).ok_or("line parse")?;

    Ok((
        caps.get(1).ok_or("line parse")?.as_str().to_owned(),
        caps.get(2).ok_or("line parse")?.as_str().parse()?,
        caps.get(3).ok_or("line parse")?.as_str().to_owned(),
        caps.get(4).ok_or("line parse")?.as_str().to_owned(),
    ))
}

fn scrape_body(body: scraper::ElementRef) -> Result<(Vec<ScheduleItem>, HashSet<String>)> {
    let mut schedules = Vec::new();
    let mut instructors = HashSet::new();
    let sched_sel = Selector::parse(
        ":scope table[summary=\
        'This table lists the scheduled meeting times and assigned instructors for this class..'\
        ] > tbody",
    )?;
    let col_sel = Selector::parse(":scope .dddefault")?;
    let instr_link_sel = Selector::parse(":scope a")?;
    let time_re = Regex::new(r"(\d?\d):(\d\d) (am|pm) - (\d?\d):(\d\d) (am|pm)")?;

    let sched_table = match body.select(&sched_sel).next() {
        Some(t) => t,
        _ => {
            return Ok((Vec::new(), HashSet::new()));
        }
    };

    for row in sched_table.select(&Selector::parse(":scope tr")?).skip(1) {
        let mut cols = row.select(&col_sel).skip(1);
        let time = cols.next().ok_or("time")?.text().next().ok_or("time")?;
        let time_tup = match time {
            "TBA" => None,
            _ => Some({
                let time_caps = time_re.captures(time).ok_or("time parse")?;

                let time_start = fix_time(&time_caps[1], &time_caps[2], &time_caps[3])?;
                let time_end = fix_time(&time_caps[4], &time_caps[5], &time_caps[6])?;
                (time_start, time_end)
            }),
        };

        let days = cols
            .next()
            .ok_or("days")?
            .inner_html()
            .decode()
            .trim()
            .chars()
            .filter_map(|d| match d {
                'M' => Some(1),
                'T' => Some(2),
                'W' => Some(3),
                'R' => Some(4),
                'F' => Some(5),
                'S' => Some(6),
                'U' => Some(7),
                _ => None,
            })
            .collect();
        let location = cols
            .next()
            .ok_or("location")?
            .text()
            .next()
            .ok_or("location text")?
            .decode();
        schedules.push(ScheduleItem {
            time: time_tup,
            days,
            location: match location.as_str() {
                "TBA" => None,
                _ => Some(location),
            },
        });

        cols.next().ok_or("date range")?;
        cols.next().ok_or("sched type")?;
        for instructor in cols.next().ok_or("instructors")?.select(&instr_link_sel) {
            instructors.insert(instructor.attr("target").ok_or("instructor name")?.into());
        }
    }

    Ok((schedules, instructors))
}

fn fix_time(hrs: &str, mins: &str, period: &str) -> Result<String> {
    Ok(match period {
        "pm" if hrs != "12" => format!("{:0>2}:{mins}", hrs.parse::<usize>()? + 12),
        _ => format!("{hrs:0>2}:{mins}"),
    })
}

trait DecodeHtmlEntities {
    // deal with some html entities we encounter in the output
    fn decode(self) -> String;
}

impl<T: Into<String>> DecodeHtmlEntities for T {
    fn decode(self) -> String {
        // chained replace isn't terribly efficient but I don't think it matters here
        self.into().replace("&amp;", "&").replace("&nbsp;", " ")
    }
}
