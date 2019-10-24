/*
Pbfextractor creates graph files for the cycle-routing projects from pbf and srtm data
Copyright (C) 2019  Florian Barth

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::ops::{Div, Mul};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Meters(pub f64);
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Kilometers(pub f64);

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Seconds(pub f64);
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Hours(pub f64);

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct MetersPerSecond(pub f64);
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct KilometersPerHour(pub f64);

impl MetersPerSecond {
    pub fn new(m: Meters, s: Seconds) -> MetersPerSecond {
        MetersPerSecond(m.0 / s.0)
    }
}

impl From<Kilometers> for Meters {
    fn from(k: Kilometers) -> Meters {
        Meters(k.0 * 1000.0)
    }
}

impl From<Hours> for Seconds {
    fn from(h: Hours) -> Self {
        Seconds(h.0 * 3600.0)
    }
}

impl From<KilometersPerHour> for MetersPerSecond {
    fn from(kmh: KilometersPerHour) -> MetersPerSecond {
        let km = Kilometers(kmh.0);
        let h = Hours(1.0);

        let m = Meters::from(km);
        let s = Seconds::from(h);

        MetersPerSecond::new(m, s)
    }
}

impl Div<MetersPerSecond> for Meters {
    type Output = Seconds;
    fn div(self, mps: MetersPerSecond) -> Self::Output {
        Seconds(self.0 / mps.0)
    }
}

impl Mul<f64> for Meters {
    type Output = Self;
    fn mul(self, c: f64) -> Self::Output {
        Meters(self.0 * c)
    }
}

#[test]
fn test_kmh_to_ms_conversion() {
    let kmh = KilometersPerHour(3.6);
    let ms = MetersPerSecond::from(kmh);

    assert_eq!(1.0, ms.0);

    let kmh = KilometersPerHour(180.0);
    let ms = MetersPerSecond::from(kmh);

    assert_eq!(50.0, ms.0);
}

#[test]
fn test_meters_div_ms() {
    let m = Meters(10.0);
    let ms = MetersPerSecond(2.0);

    assert_eq!(Seconds(5.0), m / ms);
}
