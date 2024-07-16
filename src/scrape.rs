use crate::structs::*;

use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub async fn parse(
    doc: Html,
    term: String,
    //subject_titles: HashMap<String, String>,
) -> Result<HashMap<String, Subject>> {
    let mut map = HashMap::new();
    let class_sel = Selector::parse(
        "table[summary='This layout table is used to present the sections found'] > tbody > tr",
    )?;
    let line_sel = Selector::parse(":scope > th > a")?;
    let sched_sel = Selector::parse(":scope table[summary='This table lists the scheduled meeting times and assigned instructors for this class..'] > tbody")?;
    let mut classes = doc.select(&class_sel);

    let client = Client::builder().gzip(true).build()?;

    while let (Some(line), Some(body)) = (classes.next(), classes.next()) {
        let line = line
            .select(&line_sel)
            .next()
            .ok_or("line find")?
            .inner_html();

        let (title, crn, subj, code) = parse_line(&line)?;

        let (schedules, instructors) = match body.select(&sched_sel).next() {
            Some(sched_table) => parse_sched_table(sched_table)?,
            None => (Vec::new(), HashSet::new()),
        };

        let section = Section {
            crn: crn.into(),
            instructors,
            schedules,
        };

        let courses = &mut map.entry(subj.into())
            .or_insert_with(|| {
                Subject {
                    // TODO use actual subject title
                    title: String::new(),
                    courses: HashMap::new(),
                }
            })
            .courses;

        // we have to do this instead of the more elegant element(...).or_insert_with(...)
        // because of our async catalog request
        if !courses.contains_key(code) {
            let (description, credits) = get_course_catalog(&client, &term, subj, code).await?;
            courses.insert(code.into(), Course {
                    title: title.into(),
                    description,
                    credits,
                    sections: Vec::new(),
                });
        }

        courses.get_mut(code).unwrap()
            .sections
            .push(section);
    }

    Ok(map)
}

pub async fn get_course_catalog(
    client: &Client,
    term: &str,
    subj: &str,
    code: &str,
) -> Result<(String, (String, String))> {
    let page = client
        .get("https://sis-ssb-prod.uga.edu/PROD/bwckctlg.p_disp_course_detail")
        .query(&[
            ("cat_term_in", term),
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
            (caps[1].into(), caps[2].into())
        } else if let Some(caps) = cred_single_re.captures(cred_line) {
            (caps[1].into(), caps[1].into())
        } else {
            Err("credits: no match")?
        }
    };

    Ok((desc.into(), credits))
}

fn parse_line(line: &str) -> Result<(&str, &str, &str, &str)> {
    let line_re = Regex::new(r"(.*) - (\d{5}) - ([A-Z]{4}) (\d{4}[A-Z]?) - .*")?;
    let caps = line_re.captures(line).ok_or("line parse")?;
    Ok((
        caps.get(1).ok_or("line parse")?.as_str(),
        caps.get(2).ok_or("line parse")?.as_str(),
        caps.get(3).ok_or("line parse")?.as_str(),
        caps.get(4).ok_or("line parse")?.as_str(),
    ))
}

fn parse_sched_table(sched_table: scraper::ElementRef) -> Result<(Vec<Schedule>, HashSet<String>)> {
    let mut schedules = Vec::new();
    let mut instructors = HashSet::new();
    let col_sel = Selector::parse(":scope .dddefault")?;
    let instr_link_sel = Selector::parse(":scope a")?;

    for row in sched_table.select(&Selector::parse(":scope tr")?).skip(1) {
        let mut cols = row.select(&col_sel).skip(1);
        let time = cols.next().ok_or("time")?.inner_html();
        let (time_start, time_end) = time.split_once(" - ").ok_or("time split")?;
        let days = cols.next().ok_or("days")?.inner_html();
        let location = cols.next().ok_or("location")?.inner_html();
        schedules.push(Schedule {
            times: (time_start.into(), time_end.into()),
            days,
            location,
        });

        cols.next().ok_or("date range")?;
        cols.next().ok_or("sched type")?;
        for instructor in cols.next().ok_or("instructors")?.select(&instr_link_sel) {
            instructors.insert(instructor.attr("target").ok_or("instructor name")?.into());
        }
    }

    Ok((schedules, instructors))
}
