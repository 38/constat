use chrono::{Date, Utc};

use plotters::prelude::Path as PathElement;
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
    pub fn new<P: AsRef<Path>>(
        path: P,
        data: Vec<AuthorStat>,
        back: D,
    ) -> Self {
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
            .set_label_area_size(LabelAreaPosition::Left, 60)
            .set_label_area_size(LabelAreaPosition::Bottom, 40)
            .margin(10)
            .caption(
                format!("Contributor Stat for {}", self.repo_name),
                ("Arial", 40),
            )
            .build_ranged(min_time..max_time, 0..(max_loc))
            .unwrap();

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .draw()
            .unwrap();

        let mut accumulate = HashMap::new();

        let mut last_points = vec![(min_time, 0), (max_time, 0)];

        for (i, (name, stat)) in (0..).zip(self.data.into_iter()) {
            let mut points = vec![];

            for (time, count) in stat {
                *accumulate.entry(time).or_insert(0) += count;
                points.push((time, accumulate[&time]));
            }

            let mut vert = points.clone();

            for p in last_points.iter().rev() {
                if p.0 < vert[0].0 {
                    break;
                }
                vert.push(p.clone());
            }

            let c = Palette99::pick(i);
            chart
                .draw_series(std::iter::once(Polygon::new(vert, &c.mix(0.4))))
                .unwrap();
            chart
                .draw_series(std::iter::once(PathElement::new(points.clone(), &c)))
                .unwrap()
                .label(name)
                .legend(move |(x, y)| {
                    Rectangle::new([(x, y - 5), (x + 20, y + 5)], c.mix(0.4).filled())
                });

            last_points = points;
        }

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .border_style(&BLACK)
            .draw()
            .unwrap();
    }
}
