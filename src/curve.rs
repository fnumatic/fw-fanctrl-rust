use crate::config::CurvePoint;

pub fn interpolate(curve: &[CurvePoint], temp: u32) -> u32 {
    if curve.is_empty() {
        return 0;
    }

    let mut min_point = &curve[0];
    let mut max_point = &curve[curve.len() - 1];

    for point in curve {
        if temp > point.temp {
            min_point = point;
        } else {
            max_point = point;
            break;
        }
    }

    if min_point.temp == max_point.temp {
        return min_point.speed;
    }

    let slope = (max_point.speed as i32 - min_point.speed as i32)
        / (max_point.temp as i32 - min_point.temp as i32);

    let new_speed = min_point.speed as i32 + (temp as i32 - min_point.temp as i32) * slope;

    new_speed.clamp(0, 100) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_below_min() {
        let curve = vec![
            CurvePoint {
                temp: 50,
                speed: 15,
            },
            CurvePoint {
                temp: 70,
                speed: 100,
            },
        ];
        assert_eq!(interpolate(&curve, 30), 15);
    }

    #[test]
    fn test_interpolate_above_max() {
        let curve = vec![
            CurvePoint {
                temp: 50,
                speed: 15,
            },
            CurvePoint {
                temp: 70,
                speed: 100,
            },
        ];
        assert_eq!(interpolate(&curve, 100), 100);
    }

    #[test]
    fn test_interpolate_exact_point() {
        let curve = vec![
            CurvePoint {
                temp: 50,
                speed: 15,
            },
            CurvePoint {
                temp: 70,
                speed: 100,
            },
        ];
        assert_eq!(interpolate(&curve, 50), 15);
    }

    #[test]
    fn test_interpolate_midpoint() {
        let curve = vec![
            CurvePoint {
                temp: 50,
                speed: 15,
            },
            CurvePoint {
                temp: 70,
                speed: 100,
            },
        ];
        assert_eq!(interpolate(&curve, 60), 55);
    }

    #[test]
    fn test_interpolate_empty_curve() {
        let curve: Vec<CurvePoint> = vec![];
        assert_eq!(interpolate(&curve, 50), 0);
    }

    #[test]
    fn test_interpolate_single_point() {
        let curve = vec![CurvePoint {
            temp: 50,
            speed: 50,
        }];
        assert_eq!(interpolate(&curve, 30), 50);
        assert_eq!(interpolate(&curve, 70), 50);
    }
}
