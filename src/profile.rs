use std::cell::RefCell;
use std::time::{Duration, Instant};

pub struct ProfileFrame {
    data: RefCell<ProfileData>,
}

struct ProfileData {
    active: Vec<Invocation>,
    archived: Vec<Invocation>,
}

impl ProfileData {
    pub fn new() -> Self {
        ProfileData {
            active: vec![],
            archived: vec![],
        }
    }

    pub fn begin(&mut self, name: &str) {
        let inv = Invocation {
            name: String::from(name),
            begin_time: Instant::now(),
            end_time: None,
            children: vec![],
        };
        self.active.push(inv);
    }

    pub fn end(&mut self, name: &str) {
        assert!(self.active.len() > 0, "No active invocation!");

        let mut top = self.active.remove(self.active.len() - 1);
        assert_eq!(
            top.name, name,
            "Invocation top is {}, yet trying to end {}",
            top.name, name
        );

        top.end_time = Some(Instant::now());

        if self.active.len() > 0 {
            self.active.last_mut().unwrap().children.push(top);
        } else {
            self.archived.push(top);
        }
    }
}

pub struct Guard<'a> {
    key: String,
    data: &'a RefCell<ProfileData>,
}

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        self.data.borrow_mut().end(self.key.as_str());
    }
}

impl ProfileFrame {
    pub fn new() -> ProfileFrame {
        ProfileFrame {
            data: RefCell::new(ProfileData::new()),
        }
    }

    pub fn begin(&self, name: &str) {
        self.data.borrow_mut().begin(name);
    }

    pub fn begin_guard(&self, name: &str) -> Guard {
        self.begin(name);
        Guard {
            key: String::from(name),
            data: &self.data,
        }
    }

    pub fn end(&self, name: &str) {
        self.data.borrow_mut().end(name);
    }

    pub fn dump_csv(&self) -> String {
        let mut sb = String::new();
        sb.push_str("Name,Duration(ms),Invocations,%\n");
        Self::dump_list(&mut sb, &self.data.borrow().archived, 0);

        sb
    }

    fn dump_list(sb: &mut String, ls: &Vec<Invocation>, indent: u32) {
        struct Row {
            name: String,
            duration: Duration,
            invocations: u32,
            children: Vec<Invocation>,
        }
        use itertools::Itertools;
        let mut ls2 = ls.to_vec();
        ls2.sort_by_key(|x| x.name.clone());
        let mut rows: Vec<Row> = ls2
            .into_iter()
            // group_by works like shit that it requires sorting first
            .group_by(|x| x.name.clone())
            .into_iter()
            .map(|(_, group)| {
                let group_vec: Vec<Invocation> = group.into_iter().map(|x| x.clone()).collect();
                let duration = group_vec
                    .iter()
                    .map(|x| x.end_time.unwrap() - x.begin_time)
                    .sum();
                let invocations = group_vec.len();
                let all_children: Vec<Invocation> =
                    group_vec.iter().flat_map(|x| x.children.clone()).collect();

                Row {
                    name: group_vec.first().unwrap().name.clone(),
                    duration,
                    invocations: invocations as u32,
                    children: all_children,
                }
            })
            .collect();
        rows.sort_by_key(|x| std::cmp::Reverse(x.duration));

        let total_duration: Duration = rows.iter().map(|x| x.duration).sum();
        for row in rows {
            let percentage =
                100.0f64 * ((row.duration.as_nanos() as f64) / (total_duration.as_nanos() as f64));
            for _ in 0..indent {
                sb.push_str(">>");
            }
            sb.push_str(
                format!(
                    "{},{},{},{:.2}\n",
                    row.name,
                    row.duration.as_millis(),
                    row.invocations,
                    percentage
                )
                .as_str(),
            );
            Self::dump_list(sb, &row.children, indent + 1);
        }
    }
}

#[derive(Clone)]
struct Invocation {
    name: String,
    begin_time: Instant,
    end_time: Option<Instant>,
    children: Vec<Invocation>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_profile_simple() {
        let frame = ProfileFrame::new();
        {
            let _g = frame.begin_guard("action1");
            for _ in 0..123 {
                {
                    let _g2 = frame.begin_guard("calc");
                    thread::sleep(Duration::from_micros(1));
                }
                {
                    let _g2 = frame.begin_guard("calc2");
                    thread::sleep(Duration::from_millis(2));
                }
            }
        }
        {
            let _g = frame.begin("action2");
            for _ in 0..233 {
                let _g2 = frame.begin_guard("calc");
                thread::sleep(Duration::from_millis(2));
            }
        }

        println!("{}", frame.dump_csv());
    }
}
