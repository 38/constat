use chrono::{Date, Utc, Duration};

use plotters::prelude::PathElement;
use plotters::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub type AuthorStat = (String, Vec<(Date<Utc>, usize)>);

pub struct Renderer<D: DrawingBackend> {
    data: Vec<AuthorStat>,
    repo_name: String,
    back: D,
}

impl<D: DrawingBackend> Renderer<D> {
    pub fn new<P: AsRef<Path>>(path: P, data: Vec<AuthorStat>, back: D) -> Self {
        Self {
            repo_name: path.as_ref().file_name().map_or("N/A".to_string(), |what| {
                what.to_string_lossy().into_owned()
            }),
            data,
            back,
        }
    }

    pub fn draw(self) {
        let min_time = self.data[0].1.first().unwrap().0;
        let max_time = self
            .data
            .iter()
            .map(|(_, stats)| stats.last().unwrap().0)
            .max()
            .unwrap();
        let max_loc = self
            .data
            .iter()
            .map(|(_, stats)| stats.iter().map(|x| x.1).max().unwrap())
            .sum::<usize>();

        let root = self.back.into_drawing_area();

        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(&root)
            .set_label_area_size(LabelAreaPosition::Left, (10).percent_width())
            .set_label_area_size(LabelAreaPosition::Bottom, (10).percent_height())
            .margin(10)
            .caption(
                format!("Contributor Stat for {}", self.repo_name),
                ("Arial", (5).percent_height()),
            )
            .build_ranged(min_time..max_time, 0..(max_loc))
            .unwrap();

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .draw()
            .unwrap();

        let (time_table, time_values) = {
            let mut time_values: Vec<_> = self
                .data
                .iter()
                .map(|(_, stat)| {
                    stat.iter().map(|(time, _)|{
                        let time = time.clone() - Duration::days(1);
                        (0..2).map(move |days| {
                            time.clone() + Duration::days(days)
                        })
                    }).flatten()
                })
                .flatten()
                .collect();
            time_values.sort();
            let mut j = 1;
            for i in 1..time_values.len() {
                if time_values[j - 1] != time_values[i] {
                    time_values[j] = time_values[i];
                    j += 1;
                }
            }
            time_values.truncate(j);
            (
                time_values
                    .iter()
                    .zip(0..)
                    .map(|(time, idx)| (time.clone(), idx))
                    .collect::<HashMap<_, _>>(),
                time_values,
            )
        };

        let mut accumulate = vec![0; time_values.len()];

        for (i, (name, stat)) in (0..).zip(self.data.into_iter()) {
            let mut points = vec![];
            let mut back_points = vec![];

            let end_points = stat.iter().skip(1).map(|(time, _)| time_table[time]).chain(std::iter::once(time_values.len() - 1));
            let start_points = stat.iter().map(|(time, _)| time_table[time]);
            let interval_iter = start_points.zip(end_points).enumerate().map(|(idx, (start, end))| (idx, start, end));

            for (stat_idx, start, end) in interval_iter {
                for idx in start..end {
                    back_points.push((time_values[idx], accumulate[idx]));
                    accumulate[idx] += stat[stat_idx].1;
                    points.push((time_values[idx], accumulate[idx]));
                }
            }

            let c = Palette99::pick(i);
            chart
                .draw_series(std::iter::once(Polygon::new(
                    points
                        .clone()
                        .into_iter()
                        .chain(back_points.into_iter().rev())
                        .collect::<Vec<_>>(),
                    &c.mix(0.4),
                )))
                .unwrap();
            chart
                .draw_series(std::iter::once(PathElement::new(points.clone(), &c)))
                .unwrap()
                .label(name)
                .legend(move |(x, y)| {
                    Rectangle::new([(x, y - 5), (x + 20, y + 5)], c.mix(0.4).filled())
                });
        }

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .border_style(&BLACK)
            .draw()
            .unwrap();
    }
}
