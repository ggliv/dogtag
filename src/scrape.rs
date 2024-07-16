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
) -> Result<HashMap<String, Subject>> {
    let mut map = HashMap::new();
    let client = Client::builder().gzip(true).build()?;
    let subj_titles = get_subject_titles(&client).await?;

    let class_sel = Selector::parse(
        "table[summary='This layout table is used to present the sections found'] > tbody > tr",
    )?;
    let line_sel = Selector::parse(":scope > th > a")?;
    let sched_sel = Selector::parse(":scope table[summary='This table lists the scheduled meeting times and assigned instructors for this class..'] > tbody")?;
    let mut classes = doc.select(&class_sel);

    while let (Some(line), Some(body)) = (classes.next(), classes.next()) {
        let line = line
            .select(&line_sel)
            .next()
            .ok_or("line find")?
            .inner_html();

        let (title, crn, subj, code) = parse_line(&line)?;

        let (schedule, instructors) = match body.select(&sched_sel).next() {
            Some(sched_table) => parse_sched_table(sched_table)?,
            None => (Vec::new(), HashSet::new()),
        };

        let section = Section {
            crn: crn.into(),
            instructors,
            schedule,
        };

        if !map.contains_key(subj) {
            map.insert(
                subj.into(),
                Subject {
                    title: subj_titles.get(subj).ok_or("unknown subject")?.into(),
                    courses: HashMap::new(),
                },
            );
        }

        let courses = &mut map.get_mut(subj).unwrap().courses;

        if !courses.contains_key(code) {
            let (description, credits) = get_course_catalog(&client, &term, subj, code).await?;
            courses.insert(
                code.into(),
                Course {
                    title: title.into(),
                    description,
                    credits,
                    sections: Vec::new(),
                },
            );
        }

        courses.get_mut(code).unwrap().sections.push(section);
    }

    Ok(map)
}

pub async fn get_subject_titles(client: &Client) -> Result<HashMap<String, String>> {
    let mut titles = HashMap::new();

    let page = client
        .get("https://bulletin.uga.edu/coursesHome")
        .send()
        .await?
        .text()
        .await?;
    let doc = Html::parse_document(&page);
    let subjs_sel = Selector::parse("#ddlAllPrefixes > option")?;

    for subj in doc.select(&subjs_sel).skip(1) {
        let inner = subj.inner_html();
        let (code, title) = inner.split_once(" - ").ok_or("subject split")?;
        titles.insert(code.into(), title.into());
    }

    Ok(titles)
}

pub async fn get_course_catalog(
    client: &Client,
    term: &str,
    subj: &str,
    code: &str,
) -> Result<(String, (f32, f32))> {
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
            (caps[1].parse::<f32>()?, caps[2].parse::<f32>()?)
        } else if let Some(caps) = cred_single_re.captures(cred_line) {
            (caps[1].parse::<f32>()?, caps[1].parse::<f32>()?)
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

fn parse_sched_table(sched_table: scraper::ElementRef) -> Result<(Vec<ScheduleItem>, HashSet<String>)> {
    let mut schedules = Vec::new();
    let mut instructors = HashSet::new();
    let col_sel = Selector::parse(":scope .dddefault")?;
    let instr_link_sel = Selector::parse(":scope a")?;
    let time_re = Regex::new(r"(\d?\d):(\d\d) (am|pm) - (\d?\d):(\d\d) (am|pm)")?;

    for row in sched_table.select(&Selector::parse(":scope tr")?).skip(1) {
        let mut cols = row.select(&col_sel).skip(1);
        let time = cols.next().ok_or("time")?.inner_html();
        let time_caps = time_re.captures(&time).ok_or("time parse")?;

        let time_start = fix_time(&time_caps[1], &time_caps[2], &time_caps[3])?;
        let time_end = fix_time(&time_caps[4], &time_caps[5], &time_caps[6])?;

        let days = cols.next().ok_or("days")?.inner_html().decode().trim().chars().collect();
        let location = cols.next().ok_or("location")?.inner_html().decode();
        schedules.push(ScheduleItem {
            time: (time_start, time_end),
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
