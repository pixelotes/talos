//! The unit tables of `calc.convert`: what the spec lists (docs/spec/basics.md),
//! each with an exact factor to its category's base unit, its everyday names
//! and abbreviations in both languages, and how to write it in a reply.
//!
//! Names and abbreviations are data, not sentences. A unit that is known
//! but not supported (currency, area, data rates, ...) is recognised by
//! [`Resolved`] so the reply can say why.

use talos_core::Lang;
use talos_core::normalise::fold;

use super::rat::Rat;

/// What a unit measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Category {
    Length,
    Mass,
    Temperature,
    Volume,
    Time,
    Data,
    Speed,
}

impl Category {
    /// The category's name, as vocabulary for the reply.
    pub(crate) fn name(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Category::Length, Lang::Es) => "longitud",
            (Category::Mass, Lang::Es) => "masa",
            (Category::Temperature, Lang::Es) => "temperatura",
            (Category::Volume, Lang::Es) => "volumen",
            (Category::Time, Lang::Es) => "tiempo",
            (Category::Data, Lang::Es) => "tamaño de datos",
            (Category::Speed, Lang::Es) => "velocidad",
            (Category::Length, Lang::En) => "length",
            (Category::Mass, Lang::En) => "mass",
            (Category::Temperature, Lang::En) => "temperature",
            (Category::Volume, Lang::En) => "volume",
            (Category::Time, Lang::En) => "time",
            (Category::Data, Lang::En) => "data size",
            (Category::Speed, Lang::En) => "speed",
        }
    }
}

/// One supported unit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Unit {
    /// The canonical id reported in the `from`/`to` slots.
    pub(crate) id: &'static str,
    pub(crate) category: Category,
    /// Size of one unit in the category's base unit, as `decimal` or
    /// `decimal/decimal` (temperature: unused).
    factor: &'static str,
    /// Written symbol in Spanish and English.
    symbol: [&'static str; 2],
    /// Singular and plural names: Spanish, then English.
    names: [&'static str; 4],
    /// Folded lower-case spellings, singular (a trailing "s"/"es" is also
    /// tried); the plural of a phrase is listed when it needs it.
    aliases: &'static [&'static str],
}

macro_rules! unit {
    ($id:expr, $cat:ident, $factor:expr, [$ses:expr, $sen:expr], [$a:expr, $b:expr, $c:expr, $d:expr], [$($alias:expr),* $(,)?]) => {
        Unit {
            id: $id,
            category: Category::$cat,
            factor: $factor,
            symbol: [$ses, $sen],
            names: [$a, $b, $c, $d],
            aliases: &[$($alias),*],
        }
    };
}

const UNITS: &[Unit] = &[
    // Length, base: metre.
    unit!("mm", Length, "0.001", ["mm", "mm"], ["milímetro", "milímetros", "millimeter", "millimeters"], ["mm", "milimetro", "millimeter", "millimetre"]),
    unit!("cm", Length, "0.01", ["cm", "cm"], ["centímetro", "centímetros", "centimeter", "centimeters"], ["cm", "centimetro", "centimeter", "centimetre"]),
    unit!("m", Length, "1", ["m", "m"], ["metro", "metros", "meter", "meters"], ["m", "metro", "meter", "metre"]),
    unit!("km", Length, "1000", ["km", "km"], ["kilómetro", "kilómetros", "kilometer", "kilometers"], ["km", "kilometro", "kilometer", "kilometre"]),
    unit!("in", Length, "0.0254", ["in", "in"], ["pulgada", "pulgadas", "inch", "inches"], ["in", "inch", "inches", "pulgada"]),
    unit!("ft", Length, "0.3048", ["ft", "ft"], ["pie", "pies", "foot", "feet"], ["ft", "foot", "feet", "pie", "pies"]),
    unit!("yd", Length, "0.9144", ["yd", "yd"], ["yarda", "yardas", "yard", "yards"], ["yd", "yard", "yarda"]),
    unit!("mi", Length, "1609.344", ["mi", "mi"], ["milla", "millas", "mile", "miles"], ["mi", "mile", "milla"]),
    unit!("nmi", Length, "1852", ["mn", "nmi"], ["milla náutica", "millas náuticas", "nautical mile", "nautical miles"], ["nmi", "nautical mile", "milla nautica", "milla marina"]),
    // Mass, base: gram.
    unit!("mg", Mass, "0.001", ["mg", "mg"], ["miligramo", "miligramos", "milligram", "milligrams"], ["mg", "miligramo", "milligram"]),
    unit!("g", Mass, "1", ["g", "g"], ["gramo", "gramos", "gram", "grams"], ["g", "gr", "gramo", "gram", "gramme"]),
    unit!("kg", Mass, "1000", ["kg", "kg"], ["kilogramo", "kilogramos", "kilogram", "kilograms"], ["kg", "kilo", "kilogramo", "kilogram", "kilogramme"]),
    unit!("t", Mass, "1000000", ["t", "t"], ["tonelada métrica", "toneladas métricas", "metric tonne", "metric tonnes"], ["t", "tonne", "metric tonne", "metric ton", "ton", "tonelada", "tonelada metrica"]),
    unit!("oz", Mass, "28.349523125", ["oz", "oz"], ["onza", "onzas", "ounce", "ounces"], ["oz", "ounce", "onza"]),
    unit!("lb", Mass, "453.59237", ["lb", "lb"], ["libra", "libras", "pound", "pounds"], ["lb", "lbs", "pound", "libra"]),
    // Volume, base: millilitre.
    unit!("ml", Volume, "1", ["ml", "ml"], ["mililitro", "mililitros", "milliliter", "milliliters"], ["ml", "mililitro", "milliliter", "millilitre"]),
    unit!("cl", Volume, "10", ["cl", "cl"], ["centilitro", "centilitros", "centiliter", "centiliters"], ["cl", "centilitro", "centiliter", "centilitre"]),
    unit!("l", Volume, "1000", ["l", "L"], ["litro", "litros", "liter", "liters"], ["l", "lt", "litro", "liter", "litre"]),
    unit!("floz", Volume, "29.5735295625", ["fl oz (EE. UU.)", "US fl oz"], ["onza líquida", "onzas líquidas", "fluid ounce", "fluid ounces"], ["fl oz", "floz", "fluid ounce", "onza liquida", "onza fluida"]),
    unit!("gal", Volume, "3785.411784", ["gal (EE. UU.)", "US gal"], ["galón", "galones", "gallon", "gallons"], ["gal", "galon", "gallon"]),
    // Time, base: second. Years and months are calendar averages.
    unit!("s", Time, "1", ["s", "s"], ["segundo", "segundos", "second", "seconds"], ["s", "sec", "secs", "seg", "segundo", "second"]),
    unit!("min", Time, "60", ["min", "min"], ["minuto", "minutos", "minute", "minutes"], ["min", "mins", "minuto", "minute"]),
    unit!("h", Time, "3600", ["h", "h"], ["hora", "horas", "hour", "hours"], ["h", "hr", "hrs", "hora", "hour"]),
    unit!("day", Time, "86400", ["día", "day"], ["día", "días", "day", "days"], ["day", "dia"]),
    unit!("week", Time, "604800", ["semana", "week"], ["semana", "semanas", "week", "weeks"], ["week", "semana", "wk"]),
    unit!("month", Time, "2629746", ["mes", "month"], ["mes", "meses", "month", "months"], ["month", "mes", "meses"]),
    unit!("year", Time, "31556952", ["año", "year"], ["año", "años", "year", "years"], ["year", "yr", "yrs", "ano"]),
    // Data size, base: byte. Decimal prefixes by default, binary on request.
    unit!("bit", Data, "0.125", ["bit", "bit"], ["bit", "bits", "bit", "bits"], ["bit"]),
    unit!("B", Data, "1", ["B", "B"], ["byte", "bytes", "byte", "bytes"], ["byte", "octeto"]),
    unit!("kB", Data, "1000", ["kB", "kB"], ["kilobyte", "kilobytes", "kilobyte", "kilobytes"], ["kb", "kilobyte"]),
    unit!("MB", Data, "1000000", ["MB", "MB"], ["megabyte", "megabytes", "megabyte", "megabytes"], ["mb", "megabyte", "mega"]),
    unit!("GB", Data, "1000000000", ["GB", "GB"], ["gigabyte", "gigabytes", "gigabyte", "gigabytes"], ["gb", "gigabyte", "giga"]),
    unit!("TB", Data, "1000000000000", ["TB", "TB"], ["terabyte", "terabytes", "terabyte", "terabytes"], ["tb", "terabyte", "tera"]),
    unit!("KiB", Data, "1024", ["KiB", "KiB"], ["kibibyte", "kibibytes", "kibibyte", "kibibytes"], ["kib", "kibibyte"]),
    unit!("MiB", Data, "1048576", ["MiB", "MiB"], ["mebibyte", "mebibytes", "mebibyte", "mebibytes"], ["mib", "mebibyte"]),
    unit!("GiB", Data, "1073741824", ["GiB", "GiB"], ["gibibyte", "gibibytes", "gibibyte", "gibibytes"], ["gib", "gibibyte"]),
    unit!("TiB", Data, "1099511627776", ["TiB", "TiB"], ["tebibyte", "tebibytes", "tebibyte", "tebibytes"], ["tib", "tebibyte"]),
    unit!("Kb", Data, "125", ["Kb", "Kb"], ["kilobit", "kilobits", "kilobit", "kilobits"], ["kbit", "kilobit"]),
    unit!("Mb", Data, "125000", ["Mb", "Mb"], ["megabit", "megabits", "megabit", "megabits"], ["mbit", "megabit"]),
    unit!("Gb", Data, "125000000", ["Gb", "Gb"], ["gigabit", "gigabits", "gigabit", "gigabits"], ["gbit", "gigabit"]),
    unit!("Tb", Data, "125000000000", ["Tb", "Tb"], ["terabit", "terabits", "terabit", "terabits"], ["tbit", "terabit"]),
    // Speed, base: metre per second.
    unit!("m/s", Speed, "1", ["m/s", "m/s"], ["metro por segundo", "metros por segundo", "metre per second", "metres per second"], ["m/s", "mps", "metro por segundo", "metros por segundo", "meter per second", "meters per second", "metre per second", "metres per second"]),
    unit!("km/h", Speed, "5/18", ["km/h", "km/h"], ["kilómetro por hora", "kilómetros por hora", "kilometre per hour", "kilometres per hour"], ["km/h", "kmh", "kph", "km/hr", "kilometro por hora", "kilometros por hora", "kilometer per hour", "kilometers per hour", "kilometre per hour", "kilometres per hour"]),
    unit!("mph", Speed, "0.44704", ["mph", "mph"], ["milla por hora", "millas por hora", "mile per hour", "miles per hour"], ["mph", "mi/h", "milla por hora", "millas por hora", "mile per hour", "miles per hour"]),
    unit!("kn", Speed, "463/900", ["nudo", "kn"], ["nudo", "nudos", "knot", "knots"], ["kn", "kt", "kts", "knot", "nudo"]),
];

/// Temperature scales: id, symbol, names, aliases.
const TEMPERATURES: &[Unit] = &[
    unit!("°C", Temperature, "1", ["°C", "°C"], ["grado Celsius", "grados Celsius", "degree Celsius", "degrees Celsius"], ["c", "°c", "oc", "celsius", "centigrado", "grado celsius", "grados celsius", "grados centigrados", "degree celsius", "degrees celsius", "deg c", "degrees c", "grados c", "ºc"]),
    unit!("°F", Temperature, "1", ["°F", "°F"], ["grado Fahrenheit", "grados Fahrenheit", "degree Fahrenheit", "degrees Fahrenheit"], ["f", "°f", "of", "fahrenheit", "grado fahrenheit", "grados fahrenheit", "degree fahrenheit", "degrees fahrenheit", "deg f", "degrees f", "grados f", "ºf"]),
    unit!("K", Temperature, "1", ["K", "K"], ["kelvin", "kelvin", "kelvin", "kelvin"], ["k", "°k", "kelvin", "grado kelvin", "grados kelvin", "degrees kelvin", "degrees k"]),
];

/// Bare "degrees": the scale is the one that is not the target.
const BARE_DEGREES: &[&str] = &["grado", "grados", "degree", "degrees", "deg", "°", "o"];

/// What a unit's text turned out to be.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Resolved {
    Unit(&'static Unit),
    /// "grados" / "degrees" with no scale.
    BareDegrees,
    Currency,
    Area,
    DataRate,
    /// Pressure, energy, power and the like: a unit, but not a supported
    /// category.
    OtherCategory,
    Unknown,
}

impl Unit {
    pub(crate) fn is_bit_based(&self) -> bool {
        matches!(self.id, "bit" | "Kb" | "Mb" | "Gb" | "Tb")
    }

    /// The unit as written in a reply for `amount_is_one`: symbols, except
    /// time units, which are named. `named` adds the name of a data unit in
    /// brackets, where "Mb" and "MB" must not be confused.
    pub(crate) fn display(&self, lang: Lang, plural: bool, named: bool) -> String {
        let i = usize::from(lang == Lang::En);
        if self.category == Category::Time {
            return self.name(lang, plural).to_owned();
        }
        if named && self.category == Category::Data {
            return format!("{} ({})", self.symbol[i], self.name(lang, true));
        }
        self.symbol[i].to_owned()
    }

    /// The unit's name, singular or plural.
    pub(crate) fn name(&self, lang: Lang, plural: bool) -> &'static str {
        let base = if lang == Lang::En { 2 } else { 0 };
        self.names[base + usize::from(plural)]
    }

    /// Size of one unit in the category's base unit.
    pub(crate) fn factor(&self) -> Rat {
        let parse = |text: &str| -> Rat {
            match text.split_once('.') {
                Some((int, frac)) => Rat::from_decimal(
                    false,
                    &format!("{int}{frac}"),
                    -i64::try_from(frac.len()).unwrap_or(0),
                ),
                None => Rat::from_decimal(false, text, 0),
            }
            .unwrap_or_else(|_| Rat::from_int(1))
        };
        match self.factor.split_once('/') {
            Some((n, d)) => parse(n).div(&parse(d)).unwrap_or_else(|_| Rat::from_int(1)),
            None => parse(self.factor),
        }
    }
}

/// Looks a unit up by what the user wrote (original spelling: the case of
/// "Mb" against "MB" matters for data sizes).
pub(crate) fn resolve(raw: &str) -> Resolved {
    let trimmed = raw.trim().trim_end_matches(['.', ',', '?', '!']).trim();
    // Capital K/M/G/T with a lower-case b: bits. A capital B: bytes.
    match trimmed {
        "Kb" | "Mb" | "Gb" | "Tb" => return by_id(trimmed),
        "B" => return by_id("B"),
        _ => {}
    }
    let key = clean(trimmed);
    if key.is_empty() {
        return Resolved::Unknown;
    }
    if BARE_DEGREES.contains(&key.as_str()) {
        return Resolved::BareDegrees;
    }
    let candidates = singulars(&key);
    for candidate in &candidates {
        if let Some(unit) = UNITS
            .iter()
            .chain(TEMPERATURES)
            .find(|u| u.aliases.contains(&candidate.as_str()))
        {
            return Resolved::Unit(unit);
        }
    }
    unsupported(&key)
}

fn by_id(id: &str) -> Resolved {
    UNITS
        .iter()
        .find(|u| u.id == id)
        .map_or(Resolved::Unknown, Resolved::Unit)
}

/// Lower case, no accents, "us"/"u.s." dropped, spaces collapsed, dots
/// removed ("fl. oz" is "fl oz").
fn clean(text: &str) -> String {
    let folded = fold(&text.to_lowercase());
    let folded = folded.replace('.', "");
    folded
        .split_whitespace()
        .filter(|w| !matches!(*w, "us" | "eeuu" | "estadounidense" | "estadounidenses"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The key, then without a plural ending.
fn singulars(key: &str) -> Vec<String> {
    let mut out = vec![key.to_owned()];
    if key.len() > 3 {
        for ending in ["s", "es"] {
            if let Some(stem) = key.strip_suffix(ending) {
                out.push(stem.to_owned());
            }
        }
    }
    // "millas nauticas": each word without its plural "s".
    let per_word: Vec<&str> = key
        .split(' ')
        .map(|w| if w.len() > 3 { w.strip_suffix('s').unwrap_or(w) } else { w })
        .collect();
    out.push(per_word.join(" "));
    out
}

/// Units the calculator knows it does not convert, to say so plainly.
fn unsupported(key: &str) -> Resolved {
    const CURRENCY: &[&str] = &[
        "euro", "eur", "€", "dolar", "dollar", "usd", "$", "libra esterlina", "pound sterling",
        "gbp", "£", "yen", "jpy", "peso", "bitcoin", "btc", "franco", "chf",
    ];
    const AREA: &[&str] = &["acre", "hectarea", "hectare", "ha", "are"];
    const OTHER: &[&str] = &[
        "bar", "psi", "pascal", "pa", "atm", "atmosfera", "mmhg", "joule", "julio", "j", "kj",
        "caloria", "cal", "kcal", "kwh", "wh", "watt", "vatio", "w", "kw", "hp", "cv", "mpg",
        "l/100km", "km/l", "newton", "n", "hertz", "hz",
    ];
    let words: Vec<&str> = key.split(' ').collect();
    let singular = |k: &str| singulars(k);
    if singular(key).iter().any(|k| CURRENCY.contains(&k.as_str())) {
        return Resolved::Currency;
    }
    let squared = key
        .strip_suffix('2')
        .is_some_and(|stem| stem.chars().all(|c| c.is_ascii_alphabetic()) && !stem.is_empty());
    if squared
        || words.first().is_some_and(|w| matches!(*w, "sq" | "square"))
        || key.contains("cuadrad")
        || singular(key).iter().any(|k| AREA.contains(&k.as_str()))
    {
        return Resolved::Area;
    }
    let rate = key.contains("bps")
        || key.contains("bit/s")
        || key.contains("per second") && key.contains("bit")
        || (key.ends_with("/s")
            && key.contains('b')
            && key.starts_with(['k', 'm', 'g', 't']));
    if rate {
        return Resolved::DataRate;
    }
    if singular(key).iter().any(|k| OTHER.contains(&k.as_str())) {
        return Resolved::OtherCategory;
    }
    Resolved::Unknown
}

/// `amount` of `from` in `to`, both of one category (not temperature).
pub(crate) fn convert_linear(amount: &Rat, from: &Unit, to: &Unit) -> Option<Rat> {
    amount
        .mul(&from.factor())
        .ok()?
        .div(&to.factor())
        .ok()?
        .within_limit()
        .ok()
}

/// A temperature in `from`, in Kelvin.
fn to_kelvin(amount: &Rat, from: &Unit) -> Option<Rat> {
    let ratio = |n: i64, d: i64| Rat::from_int(n).div(&Rat::from_int(d)).ok();
    let kelvin_offset = Rat::from_decimal(false, "27315", -2).ok()?;
    match from.id {
        "°C" => amount.add(&kelvin_offset).ok(),
        "°F" => amount
            .sub(&Rat::from_int(32))
            .ok()?
            .mul(&ratio(5, 9)?)
            .ok()?
            .add(&kelvin_offset)
            .ok(),
        _ => Some(amount.clone()),
    }
}

/// A temperature in Kelvin, in `to`.
fn from_kelvin(kelvin: &Rat, to: &Unit) -> Option<Rat> {
    let ratio = |n: i64, d: i64| Rat::from_int(n).div(&Rat::from_int(d)).ok();
    let kelvin_offset = Rat::from_decimal(false, "27315", -2).ok()?;
    match to.id {
        "°C" => kelvin.sub(&kelvin_offset).ok(),
        "°F" => kelvin
            .sub(&kelvin_offset)
            .ok()?
            .mul(&ratio(9, 5)?)
            .ok()?
            .add(&Rat::from_int(32))
            .ok(),
        _ => Some(kelvin.clone()),
    }
}

/// Outcome of a temperature conversion.
pub(crate) enum Temperature {
    Value(Rat),
    /// The starting temperature is below absolute zero.
    BelowZero,
}

pub(crate) fn convert_temperature(amount: &Rat, from: &Unit, to: &Unit) -> Option<Temperature> {
    let kelvin = to_kelvin(amount, from)?;
    if kelvin.is_neg() {
        return Some(Temperature::BelowZero);
    }
    from_kelvin(&kelvin, to).map(Temperature::Value)
}

/// The temperature scale to take for a bare "degrees" given the target:
/// the one of the three that is not the target (Celsius for Kelvin).
pub(crate) fn assumed_scale(target: &Unit) -> Option<&'static Unit> {
    let id = match target.id {
        "°F" | "K" => "°C",
        "°C" => "°F",
        _ => return None,
    };
    TEMPERATURES.iter().find(|u| u.id == id)
}

/// The binary twin of a decimal data unit (GB -> GiB), when it has one.
pub(crate) fn binary_twin(unit: &Unit) -> Option<&'static Unit> {
    let id = match unit.id {
        "kB" => "KiB",
        "MB" => "MiB",
        "GB" => "GiB",
        "TB" => "TiB",
        _ => return None,
    };
    UNITS.iter().find(|u| u.id == id)
}

/// Whether a conversion involves a month or a year on one side only, so the
/// day count uses the calendar average.
pub(crate) fn uses_average_calendar(from: &Unit, to: &Unit) -> bool {
    let long = |u: &Unit| matches!(u.id, "month" | "year");
    from.category == Category::Time && to.category == Category::Time && (long(from) != long(to))
}

#[cfg(test)]
fn factor_is_positive(unit: &Unit) -> bool {
    !unit.factor().is_zero() && !unit.factor().is_neg()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> Option<&'static str> {
        match resolve(raw) {
            Resolved::Unit(u) => Some(u.id),
            _ => None,
        }
    }

    #[test]
    fn everyday_names_and_abbreviations_resolve() {
        let table = [
            ("km", "km"),
            ("Kilómetros", "km"),
            ("millas", "mi"),
            ("miles", "mi"),
            ("pulgadas", "in"),
            ("inches", "in"),
            ("feet", "ft"),
            ("pies", "ft"),
            ("libras", "lb"),
            ("pounds", "lb"),
            ("kilos", "kg"),
            ("gramos", "g"),
            ("litros", "l"),
            ("liters", "l"),
            ("galones", "gal"),
            ("US gallons", "gal"),
            ("fl oz", "floz"),
            ("fl. oz", "floz"),
            ("segundos", "s"),
            ("horas", "h"),
            ("días", "day"),
            ("años", "year"),
            ("megas", "MB"),
            ("gb", "GB"),
            ("GiB", "GiB"),
            ("Mb", "Mb"),
            ("mb", "MB"),
            ("B", "B"),
            ("km/h", "km/h"),
            ("millas por hora", "mph"),
            ("knots", "kn"),
            ("celsius", "°C"),
            ("°F", "°F"),
            ("c", "°C"),
            ("grados fahrenheit", "°F"),
            ("millas náuticas", "nmi"),
            ("nautical miles", "nmi"),
            ("toneladas", "t"),
        ];
        for (raw, want) in table {
            assert_eq!(id(raw), Some(want), "{raw}");
        }
    }

    #[test]
    fn known_but_unsupported_and_unknown_units() {
        assert!(matches!(resolve("euros"), Resolved::Currency));
        assert!(matches!(resolve("dólares"), Resolved::Currency));
        assert!(matches!(resolve("m2"), Resolved::Area));
        assert!(matches!(resolve("ft2"), Resolved::Area));
        assert!(matches!(resolve("metros cuadrados"), Resolved::Area));
        assert!(matches!(resolve("Mbps"), Resolved::DataRate));
        assert!(matches!(resolve("MB/s"), Resolved::DataRate));
        assert!(matches!(resolve("psi"), Resolved::OtherCategory));
        assert!(matches!(resolve("flurbos"), Resolved::Unknown));
        assert!(matches!(resolve("grados"), Resolved::BareDegrees));
        assert!(matches!(resolve("degrees"), Resolved::BareDegrees));
        assert!(matches!(resolve(""), Resolved::Unknown));
    }

    #[test]
    fn every_factor_parses_to_a_positive_number() {
        for unit in UNITS.iter().chain(TEMPERATURES) {
            assert!(factor_is_positive(unit), "{}", unit.id);
        }
    }

    #[test]
    fn linear_conversions_are_exact() {
        let by = |s: &str| match resolve(s) {
            Resolved::Unit(u) => u,
            _ => panic!("{s}"),
        };
        let five = Rat::from_int(5);
        let cm = convert_linear(&five, by("in"), by("cm")).unwrap();
        assert_eq!(cm, Rat::from_decimal(false, "127", -1).unwrap());
        let m = convert_linear(&Rat::from_int(5), by("nmi"), by("m")).unwrap();
        assert_eq!(m, Rat::from_int(9260));
        let two_years = convert_linear(&Rat::from_int(2), by("year"), by("day")).unwrap();
        assert_eq!(two_years, Rat::from_decimal(false, "730485", -3).unwrap());
        assert_eq!(
            convert_linear(&Rat::from_int(1), by("year"), by("month")).unwrap(),
            Rat::from_int(12)
        );
        assert_eq!(
            convert_linear(&Rat::from_int(8), by("Mb"), by("MB")).unwrap(),
            Rat::from_int(1)
        );
        assert_eq!(
            convert_linear(&Rat::from_int(2), by("GiB"), by("MiB")).unwrap(),
            Rat::from_int(2048)
        );
        assert_eq!(
            convert_linear(&Rat::from_int(36), by("km/h"), by("m/s")).unwrap(),
            Rat::from_int(10)
        );
    }

    #[test]
    fn temperatures_convert_through_kelvin() {
        let by = |s: &str| match resolve(s) {
            Resolved::Unit(u) => u,
            _ => panic!("{s}"),
        };
        let value = |amount: i64, from: &str, to: &str| match convert_temperature(
            &Rat::from_int(amount),
            by(from),
            by(to),
        ) {
            Some(Temperature::Value(v)) => Some(v),
            _ => None,
        };
        assert_eq!(value(25, "c", "f"), Some(Rat::from_int(77)));
        assert_eq!(value(-40, "c", "f"), Some(Rat::from_int(-40)));
        assert_eq!(value(0, "c", "k"), Rat::from_decimal(false, "27315", -2).ok());
        assert_eq!(value(212, "f", "c"), Some(Rat::from_int(100)));
        assert!(matches!(
            convert_temperature(&Rat::from_int(-300), by("c"), by("k")),
            Some(Temperature::BelowZero)
        ));
        assert!(matches!(
            convert_temperature(&Rat::from_int(-460), by("f"), by("c")),
            Some(Temperature::BelowZero)
        ));
    }
}
