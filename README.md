# dogtag
Web scraper for [the University of Georgia's online course catalog](https://sis-ssb-prod.uga.edu/PROD/bwckschd.p_disp_dyn_sched).

This tool gets:
- Course title
- Subject code
- Course code
- Instructors
- Description
- Credit hours
- Terms
- CRNs
- Class schedules

## Configuration
The config environment variable prefix is `DT_`. For example, if you wish to set the key "foo" to the value "bar", you would run the program with `DT_FOO=bar` in the environment.

Here are the values you can configure:
| Key | Description |
| --- | ----------- |
| base_domain | The base domain of the online catalog. e.g. `sis-ssb-prod.uga.edu`. |
| home_url | The path to the home page for the catalog. e.g. `/PROD/bwckschd.p_disp_dyn_sched` |
| term | The term to scrape for. Should be one of the options that show up on the home page, e.g "Fall 2024", "Spring 2023", etc. |

## To run
First, make sure you have all configuration variables set up as described above. Then, run:
```sh
cargo run
```

## Output structure
```json
{
   "{subjectCode}":{
      "title":"Long name of subject",
      "courses":{
         "{courseNumber}":{
            "title":"Long name of course",
            "description":"Description of course",
            "credits":[
               "{lowerBound}",
               "{upperBound}"
            ],
            "sections":[
               {
                  "crn":"crn",
                  "instructors":[
                     "{lastName}, {firstName}",
                     "..."
                  ],
                  "schedule":[
                    "times":["{timeStart}", "{timeEnd}"],
                    "days":"{days}",
                    "location":"{location}"
                  ]
               },
               "..."
            ]
         },
         "...":"etc"
      }
   },
   "...":"etc"
}
```

## TODO
- ratelimiting requests to the catalog
