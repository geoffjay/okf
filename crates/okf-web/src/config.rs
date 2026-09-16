//! Per-bundle site configuration: `.okf/config.yaml`.
//!
//! A bundle pins its site settings the way it pins its content — in a file
//! inside the bundle. `.okf/` is invisible to every OKF walker (the loader
//! collects `*.md` only), so configuration and font files never become
//! concepts, never appear in `okf validate`/`okf lint`/`okf index` output,
//! and never enter the link graph. This is tool configuration, the same
//! category as `.git/`.
//!
//! ```yaml
//! # .okf/config.yaml
//! site:
//!   title: "Travel knowledge"
//!   fonts:
//!     body: "Newsreader, Georgia, serif"
//!     code: "JetBrains Mono, ui-monospace, monospace"
//!     files:
//!       - family: Newsreader
//!         file: newsreader-400.woff2
//!         weight: 400
//!       - family: Newsreader
//!         file: newsreader-700i.woff2
//!         weight: 700
//!         style: italic
//!   theme: nord
//!   themes:
//!     - id: nord
//!       label: "Nord"
//!       scheme: dark
//!       colors:
//!         surface: "#2e3440"
//!         ink: "#eceff4"
//! ```
//!
//! ## Permissive content, strict tooling
//!
//! Bundles stay permissive: a frontmatter parse error or a broken link
//! renders as visible content. Configuration is the opposite, because a typo
//! that silently does nothing is hostile in CI.
//!
//! - A **missing** file is not an error: it yields [`SiteConfig::default`],
//!   and the generated site is byte-identical to a pre-configuration world.
//! - An **unreadable or malformed** file is an error, carrying the YAML line.
//! - An **unknown key** inside a known section is an error naming the key and
//!   the section's known keys.
//! - An **unknown section** is ignored with a note ([`SiteConfig::notes`]), so
//!   a config written for a newer `okf` degrades instead of breaking.
//!
//! ## CSS is constructed, never interpolated
//!
//! No configured string reaches the stylesheet verbatim. Family lists parse
//! into segments — a generic keyword from a closed list, or a family name
//! restricted to letters, digits, spaces, `-`, `_`, `.`, and `+` — and are
//! re-emitted with names quoted and keywords bare; weights are numbers;
//! styles come from a closed vocabulary; `url()` targets are
//! generator-built paths of files the generator copied itself, named by a
//! flat charset that makes `../` a parse error rather than a runtime check.
//! Themes are held to the same standard, one level up: a theme id matches
//! `[a-z][a-z0-9-]{0,31}`, so it can never close an attribute selector or a
//! block; color token names come from the closed [`THEME_TOKENS`] set, so a
//! theme can neither invent a custom property nor reach the font tokens; and
//! color values parse against a closed grammar — `#rgb`, `#rgba`,
//! `#rrggbb`, `#rrggbbaa`, the keyword `transparent`, and
//! `rgb()`/`rgba()`/`hsl()`/`hsla()`/`oklch()`/`oklab()`/`lab()`/`lch()`
//! over numeric, percentage, angle, and `none` arguments — then re-emit
//! from the parse. Nesting, `var()`, `url()`, quotes, and `;` are parse
//! errors, not filtered strings.
//!
//! Emission therefore lives here, beside the validation it relies on: see
//! [`Fonts::to_css`] and [`Themes::to_css`].

use okf_core::yaml::{Mapping, Value, YamlError};
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The configuration file's bundle-relative path, as written in
/// documentation and error messages. [`config_path`] builds the real path.
pub const CONFIG_PATH: &str = ".okf/config.yaml";

/// The bundle-relative directory holding self-hosted font files, as written
/// in documentation and error messages. [`fonts_dir`] builds the real path.
pub const FONTS_DIR: &str = ".okf/fonts";

/// The configuration file inside the bundle at `root`.
#[must_use]
pub fn config_path(root: &Path) -> PathBuf {
    root.join(".okf").join("config.yaml")
}

/// The self-hosted font directory inside the bundle at `root`.
#[must_use]
pub fn fonts_dir(root: &Path) -> PathBuf {
    root.join(".okf").join("fonts")
}

/// A bundle's `.okf/config.yaml`, reduced to what `okf site` consumes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SiteConfig {
    /// The file this configuration came from, absent when the bundle has no
    /// `.okf/config.yaml`. Reported by the build, because "the file the
    /// generator actually read" is the one fact that separates a config that
    /// took effect from one nothing ever saw — a misplaced file, a bundle
    /// root one directory up, an `okf` predating the feature.
    pub path: Option<PathBuf>,
    /// `site.title` — the header title, overridden by `okf site --title`.
    pub title: Option<String>,
    /// `site.fonts` — the typography token overrides and self-hosted faces.
    pub fonts: Fonts,
    /// `site.theme` — the theme a first-time visitor gets, overridden by
    /// `okf site --theme`. [`AUTO_THEME_ID`] means "follow the system".
    /// Whether the id names a theme the site has is the build's question,
    /// not this parser's: the catalog is the built-ins plus [`Self::themes`].
    pub theme: Option<String>,
    /// `site.themes` — the palettes this bundle adds to the built-in
    /// catalog, emitted as `assets/theme.css`.
    pub themes: Themes,
    /// Sections the file carries that this version of `okf` does not know,
    /// reported by the build rather than rejected.
    pub notes: Vec<String>,
}

impl SiteConfig {
    /// Loads the configuration for the bundle at `root`.
    ///
    /// A missing file yields [`SiteConfig::default`]; see the module docs for
    /// the strictness rules that apply once the file exists.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] if the file exists but cannot be read, is
    /// not YAML this parser accepts, or carries a key, type, or value
    /// `okf site` does not know.
    pub fn load(root: &Path) -> Result<Self, ConfigError> {
        let path = config_path(root);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(ConfigError::new(path, ConfigErrorKind::Read(e))),
        };
        let mut config = Self::parse(&text).map_err(|kind| ConfigError::new(path.clone(), kind))?;
        config.path = Some(path);
        Ok(config)
    }

    /// Which `site:` settings this configuration supplies, in schema order —
    /// `title`, `fonts.body`, `fonts.code`, `fonts.files`, `theme`, and
    /// `themes` — the build reports them so an ignored key or an unset
    /// section is visible without re-reading the file.
    #[must_use]
    pub fn settings(&self) -> Vec<&'static str> {
        let mut settings = Vec::with_capacity(6);
        if self.title.is_some() {
            settings.push("title");
        }
        if self.fonts.body.is_some() {
            settings.push("fonts.body");
        }
        if self.fonts.code.is_some() {
            settings.push("fonts.code");
        }
        if !self.fonts.files.is_empty() {
            settings.push("fonts.files");
        }
        if self.theme.is_some() {
            settings.push("theme");
        }
        if !self.themes.is_empty() {
            settings.push("themes");
        }
        settings
    }

    /// Parses configuration text (the file body).
    fn parse(text: &str) -> Result<Self, ConfigErrorKind> {
        let document = Value::parse(text).map_err(ConfigErrorKind::Yaml)?;
        let mut config = Self::default();
        let sections = match &document {
            // An empty or comment-only file is a file that configures nothing.
            Value::Null => return Ok(config),
            Value::Mapping(map) => map,
            _ => {
                return Err(invalid(
                    "the document must be a mapping of sections (e.g. `site:`)",
                ));
            }
        };
        if sections.iter().any(|(key, _)| key.as_str().is_none()) {
            return Err(invalid("section names must be strings"));
        }
        for (name, value) in sections.iter() {
            let name = name.as_str().unwrap_or_default();
            if name == "site" {
                config.read_site(value)?;
            } else {
                config
                    .notes
                    .push(format!("{CONFIG_PATH}: ignoring unknown section `{name}`"));
            }
        }
        Ok(config)
    }

    /// Reads the `site:` section into `self`.
    fn read_site(&mut self, value: &Value) -> Result<(), ConfigErrorKind> {
        const KNOWN: &[&str] = &["title", "fonts", "theme", "themes"];

        let Some(map) = mapping_or_empty(value, "site")? else {
            return Ok(());
        };
        check_keys(map, "site", KNOWN)?;
        match map.get("title") {
            None | Some(Value::Null) => {}
            Some(value) => {
                let title = value
                    .as_display_str()
                    .ok_or_else(|| invalid("`site.title` must be a string"))?;
                if title.trim().is_empty() {
                    return Err(invalid("`site.title` must not be empty"));
                }
                self.title = Some(title.into_owned());
            }
        }
        if let Some(value) = map.get("fonts") {
            self.fonts = Fonts::read(value)?;
        }
        match map.get("theme") {
            None | Some(Value::Null) => {}
            Some(value) => {
                let theme = value
                    .as_str()
                    .ok_or_else(|| invalid("`site.theme` must be a string"))?
                    .trim();
                if theme.is_empty() {
                    return Err(invalid("`site.theme` must not be empty"));
                }
                if theme != AUTO_THEME_ID {
                    theme_id(theme, "site.theme")?;
                }
                self.theme = Some(theme.to_string());
            }
        }
        if let Some(value) = map.get("themes") {
            self.themes = Themes::read(value)?;
        }
        Ok(())
    }
}

/// The banner both generated stylesheets open with: what wrote the file,
/// why editing it is pointless, and why its rules win the cascade.
const GENERATED_HEADER: &str = "/* Generated by `okf site` from `.okf/config.yaml`. Do not edit:\n   \
     rebuild the site instead. Linked after the inline stylesheet, so\n   \
     these token overrides win the cascade. */\n";

/// The `site.fonts` group: the two typography tokens plus self-hosted faces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fonts {
    /// `site.fonts.body` — the `--font-body` override.
    pub body: Option<FamilyList>,
    /// `site.fonts.code` — the `--font-code` override.
    pub code: Option<FamilyList>,
    /// `site.fonts.files` — faces served from `assets/fonts/`.
    pub files: Vec<FontFile>,
}

impl Fonts {
    /// `true` when the bundle configures nothing about fonts, in which case
    /// the build writes no `assets/fonts.css` and links nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.body.is_none() && self.code.is_none() && self.files.is_empty()
    }

    /// The generated `assets/fonts.css`: an `@font-face` per configured file
    /// followed by a `:root` block overriding the typography tokens.
    ///
    /// Every byte is constructed from validated values (see the module
    /// docs). `url()` targets are stylesheet-relative — `fonts.css` and the
    /// copied faces both live under `assets/`, so one href works at every
    /// page depth. `font-display: swap` keeps text visible while a face
    /// loads, which matters most on the first paint of a cold cache.
    #[must_use]
    pub fn to_css(&self) -> String {
        let mut css = String::with_capacity(220 + self.files.len() * 160);
        css.push_str(GENERATED_HEADER);
        for file in &self.files {
            let _ = write!(
                css,
                "\n@font-face {{\n  \
                 font-family: \"{family}\";\n  \
                 src: url(\"fonts/{file_name}\") format(\"{format}\");\n  \
                 font-style: {style};\n",
                family = file.family,
                file_name = file.file,
                format = file.format.as_css(),
                style = file.style.as_css(),
            );
            if let Some(weight) = file.weight {
                let _ = writeln!(css, "  font-weight: {weight};");
            }
            css.push_str("  font-display: swap;\n}\n");
        }
        if self.body.is_some() || self.code.is_some() {
            css.push_str("\n:root {\n");
            if let Some(body) = &self.body {
                let _ = writeln!(css, "  --font-body: {};", body.as_css());
            }
            if let Some(code) = &self.code {
                let _ = writeln!(css, "  --font-code: {};", code.as_css());
            }
            css.push_str("}\n");
        }
        css
    }

    /// Reads the `site.fonts` mapping.
    fn read(value: &Value) -> Result<Self, ConfigErrorKind> {
        const KNOWN: &[&str] = &["body", "code", "files"];

        let Some(map) = mapping_or_empty(value, "site.fonts")? else {
            return Ok(Self::default());
        };
        check_keys(map, "site.fonts", KNOWN)?;
        let files = match map.get("files") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Sequence(items)) => items
                .iter()
                .enumerate()
                .map(|(index, item)| FontFile::read(item, index))
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(invalid("`site.fonts.files` must be a list")),
        };
        Ok(Self {
            body: FamilyList::read(map.get("body"), "site.fonts.body")?,
            code: FamilyList::read(map.get("code"), "site.fonts.code")?,
            files,
        })
    }
}

/// A validated CSS `font-family` list, stored as the CSS text it emits.
///
/// The text is constructed, never copied from the config: each
/// comma-separated segment is trimmed and validated, then re-emitted with
/// generic keywords bare and family names double-quoted. A value that cannot
/// be built this way is a configuration error, so the stylesheet can never
/// inherit a stray quote, brace, or semicolon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyList(String);

/// The CSS generic font families — the closed set of segments emitted
/// unquoted. From CSS Fonts 4; anything else is a family name.
const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
    "fangsong",
];

impl FamilyList {
    /// The CSS text, e.g. `"Newsreader", "Georgia", serif`.
    #[must_use]
    pub fn as_css(&self) -> &str {
        &self.0
    }

    /// Reads an optional family list at configuration path `at`.
    fn read(value: Option<&Value>, at: &str) -> Result<Option<Self>, ConfigErrorKind> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(value) => {
                let raw = value
                    .as_str()
                    .ok_or_else(|| invalid(format!("`{at}` must be a string")))?;
                Self::parse(raw, at).map(Some)
            }
        }
    }

    /// Parses `Newsreader, Georgia, serif` into its CSS form.
    fn parse(raw: &str, at: &str) -> Result<Self, ConfigErrorKind> {
        let mut css = String::with_capacity(raw.len() + 8);
        for segment in raw.split(',') {
            let segment = segment.trim();
            if segment.is_empty() {
                return Err(invalid(format!(
                    "`{at}` has an empty family (check the commas in {raw:?})"
                )));
            }
            if !css.is_empty() {
                css.push_str(", ");
            }
            if GENERIC_FAMILIES.contains(&segment) {
                css.push_str(segment);
            } else {
                css.push('"');
                css.push_str(family_name(segment, at)?);
                css.push('"');
            }
        }
        if css.is_empty() {
            return Err(invalid(format!("`{at}` must name at least one family")));
        }
        Ok(Self(css))
    }
}

/// One self-hosted face: a file under [`FONTS_DIR`] plus the `@font-face`
/// descriptors it is served with.
///
/// Fields are private because their values are CSS: only [`SiteConfig::load`]
/// builds one, and it only builds validated ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFile {
    family: String,
    file: String,
    format: FontFormat,
    weight: Option<u16>,
    style: FontStyle,
}

impl FontFile {
    /// The `font-family` this face is served under.
    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    /// The file name, flat and extension-checked: the same name under
    /// [`FONTS_DIR`] in the bundle and under `assets/fonts/` in the site.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The `@font-face` format hint, derived from the file extension.
    #[must_use]
    pub const fn format(&self) -> FontFormat {
        self.format
    }

    /// The `font-weight` descriptor, if configured.
    #[must_use]
    pub const fn weight(&self) -> Option<u16> {
        self.weight
    }

    /// The `font-style` descriptor; `normal` unless configured.
    #[must_use]
    pub const fn style(&self) -> FontStyle {
        self.style
    }

    /// Reads one `site.fonts.files` entry.
    fn read(value: &Value, index: usize) -> Result<Self, ConfigErrorKind> {
        const KNOWN: &[&str] = &["family", "file", "weight", "style"];

        let at = format!("site.fonts.files[{index}]");
        let map = value
            .as_mapping()
            .ok_or_else(|| invalid(format!("`{at}` must be a mapping with `family` and `file`")))?;
        check_keys(map, &at, KNOWN)?;
        let family = required_str(map, "family", &at)?;
        let file = required_str(map, "file", &at)?;
        let (file, format) = font_file_name(file, &format!("{at}.file"))?;
        Ok(Self {
            family: family_name(family, &format!("{at}.family"))?.to_string(),
            file,
            format,
            weight: read_weight(map.get("weight"), &at)?,
            style: FontStyle::read(map.get("style"), &at)?,
        })
    }
}

/// The font file formats `okf site` serves, one per accepted extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFormat {
    /// `.woff2` — `format("woff2")`.
    Woff2,
    /// `.woff` — `format("woff")`.
    Woff,
    /// `.ttf` — `format("truetype")`.
    Ttf,
    /// `.otf` — `format("opentype")`.
    Otf,
}

impl FontFormat {
    /// The `@font-face` `format()` hint.
    #[must_use]
    pub const fn as_css(self) -> &'static str {
        match self {
            Self::Woff2 => "woff2",
            Self::Woff => "woff",
            Self::Ttf => "truetype",
            Self::Otf => "opentype",
        }
    }

    /// The format for a file extension, or `None` for an extension
    /// `okf site` does not serve.
    fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "woff2" => Some(Self::Woff2),
            "woff" => Some(Self::Woff),
            "ttf" => Some(Self::Ttf),
            "otf" => Some(Self::Otf),
            _ => None,
        }
    }
}

/// The `font-style` descriptors a face may be configured with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontStyle {
    /// The default.
    #[default]
    Normal,
    /// A true italic face.
    Italic,
    /// A slanted face.
    Oblique,
}

impl FontStyle {
    /// The `font-style` value.
    #[must_use]
    pub const fn as_css(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique => "oblique",
        }
    }

    /// Reads an optional `style` key.
    fn read(value: Option<&Value>, at: &str) -> Result<Self, ConfigErrorKind> {
        match value {
            None | Some(Value::Null) => Ok(Self::Normal),
            Some(value) => match value.as_str() {
                Some("normal") => Ok(Self::Normal),
                Some("italic") => Ok(Self::Italic),
                Some("oblique") => Ok(Self::Oblique),
                _ => Err(invalid(format!(
                    "`{at}.style` must be one of: normal, italic, oblique"
                ))),
            },
        }
    }
}

/// The `site.themes` list: the palettes a bundle adds to the built-in
/// catalog, in the order the file declares them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Themes(Vec<Theme>);

impl Themes {
    /// `true` when the bundle defines no themes, in which case the build
    /// writes no `assets/theme.css` and links nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many themes the bundle defines.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// The themes, in configuration order — the order the picker lists them
    /// in, after the built-in entries.
    pub fn iter(&self) -> std::slice::Iter<'_, Theme> {
        self.0.iter()
    }

    /// The generated `assets/theme.css`: one `:root[data-theme="…"]` block
    /// per theme that overrides at least one token.
    ///
    /// Declarations are emitted in [`THEME_TOKENS`] order rather than the
    /// author's, so one palette always produces one set of bytes. A theme
    /// that declares only a scheme contributes no block: it *is* the base
    /// palette its scheme selects, and that palette is already compiled into
    /// the inline stylesheet. Every byte is constructed from validated
    /// values (see the module docs).
    #[must_use]
    pub fn to_css(&self) -> String {
        let mut css = String::with_capacity(GENERATED_HEADER.len() + self.0.len() * 200);
        css.push_str(GENERATED_HEADER);
        for theme in &self.0 {
            if theme.colors.is_empty() {
                continue;
            }
            let _ = write!(css, "\n:root[data-theme=\"{}\"] {{\n", theme.id);
            for (token, color) in &theme.colors {
                let _ = writeln!(css, "  --{token}: {};", color.as_css());
            }
            css.push_str("}\n");
        }
        css
    }

    /// Reads the `site.themes` list.
    fn read(value: &Value) -> Result<Self, ConfigErrorKind> {
        let themes = match value {
            Value::Null => Vec::new(),
            Value::Sequence(items) => items
                .iter()
                .enumerate()
                .map(|(index, item)| Theme::read(item, index))
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(invalid("`site.themes` must be a list")),
        };
        for (index, theme) in themes.iter().enumerate() {
            if themes[..index].iter().any(|prior| prior.id == theme.id) {
                return Err(invalid(format!(
                    "`site.themes[{index}]`: duplicate theme id `{}`; each id \
                     names one palette",
                    theme.id
                )));
            }
        }
        Ok(Self(themes))
    }
}

impl<'a> IntoIterator for &'a Themes {
    type Item = &'a Theme;
    type IntoIter = std::slice::Iter<'a, Theme>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// The theme id meaning "follow the system preference".
///
/// It is the catalog's first entry and the behavior of a site that
/// configures nothing. A bundle may select it with `site.theme`, but may
/// not define a theme with it.
pub const AUTO_THEME_ID: &str = "auto";

/// The palette tokens a bundle theme may override, in emission order.
///
/// A closed set, and deliberately not the whole token vocabulary: the
/// typography tokens belong to `site.fonts`, and keeping the two disjoint is
/// what makes "a theme cannot restyle the fonts" a parse-time fact rather
/// than a convention.
pub const THEME_TOKENS: [&str; 19] = [
    "surface",
    "ink",
    "edge",
    "link",
    "shadow",
    "panel-bg",
    "code-bg",
    "mermaid-bg",
    "tier-unverified",
    "tier-machine",
    "tier-human",
    "status-draft",
    "status-stable",
    "status-deprecated",
    "status-other",
    "sev-info",
    "sev-warning",
    "sev-error",
    "danger",
];

/// One bundle-defined palette: an identity, the light/dark base it sits on,
/// and the tokens it overrides.
///
/// Fields are private because their values are CSS: only [`SiteConfig::load`]
/// builds one, and it only builds validated ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    id: String,
    label: Option<String>,
    scheme: Scheme,
    colors: Vec<(&'static str, Color)>,
}

impl Theme {
    /// The id: the `data-theme` value, the persisted choice, and what
    /// `site.theme` names. Matches `[a-z][a-z0-9-]{0,31}`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The picker's menu text — the id when the theme configures no `label`.
    /// It is HTML, escaped like every other interpolated string, not CSS.
    #[must_use]
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }

    /// The light/dark base the palette overrides.
    #[must_use]
    pub const fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// Reads one `site.themes` entry.
    fn read(value: &Value, index: usize) -> Result<Self, ConfigErrorKind> {
        const KNOWN: &[&str] = &["id", "label", "scheme", "colors"];

        let at = format!("site.themes[{index}]");
        let map = value
            .as_mapping()
            .ok_or_else(|| invalid(format!("`{at}` must be a mapping with `id` and `scheme`")))?;
        check_keys(map, &at, KNOWN)?;
        let id = required_str(map, "id", &at)?;
        if id == AUTO_THEME_ID {
            return Err(invalid(format!(
                "`{at}.id`: `{AUTO_THEME_ID}` is reserved for the built-in \
                 follow-the-system entry; choose another id"
            )));
        }
        let label = match map.get("label") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let label = value
                    .as_str()
                    .ok_or_else(|| invalid(format!("`{at}.label` must be a string")))?
                    .trim();
                if label.is_empty() {
                    return Err(invalid(format!("`{at}.label` must not be empty")));
                }
                Some(label.to_string())
            }
        };
        Ok(Self {
            id: theme_id(id, &format!("{at}.id"))?.to_string(),
            label,
            scheme: Scheme::read(map.get("scheme"), &at)?,
            colors: Self::read_colors(map.get("colors"), &at)?,
        })
    }

    /// Reads the optional `colors` mapping into [`THEME_TOKENS`] order, so
    /// the emitted block does not depend on how the author sorted the keys.
    fn read_colors(
        value: Option<&Value>,
        at: &str,
    ) -> Result<Vec<(&'static str, Color)>, ConfigErrorKind> {
        let Some(value) = value else {
            return Ok(Vec::new());
        };
        let at = format!("{at}.colors");
        let Some(map) = mapping_or_empty(value, &at)? else {
            return Ok(Vec::new());
        };
        check_keys(map, &at, &THEME_TOKENS)?;
        let mut colors = Vec::new();
        for token in THEME_TOKENS {
            match map.get(token) {
                None | Some(Value::Null) => {}
                Some(value) => colors.push((token, Color::read(value, &format!("{at}.{token}"))?)),
            }
        }
        Ok(colors)
    }
}

/// The light/dark axis a theme sits on: the base palette it overrides, the
/// `color-scheme` the page declares, the side of the code theme the
/// highlighter resolves, and which glyph the picker shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// A light palette, over the stylesheet's `:root` base.
    Light,
    /// A dark palette, over the stylesheet's `[data-scheme="dark"]` base.
    Dark,
}

impl Scheme {
    /// The `data-scheme` attribute value.
    #[must_use]
    pub const fn as_css(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Reads the required `scheme` key of the theme at configuration path
    /// `at`. It is required because it cannot be inferred: with more than
    /// two themes, "is this one dark?" stops being a question about the id.
    fn read(value: Option<&Value>, at: &str) -> Result<Self, ConfigErrorKind> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Null)) else {
            return Err(invalid(format!(
                "`{at}` needs a `scheme` of `light` or `dark`"
            )));
        };
        match value.as_str() {
            Some("light") => Ok(Self::Light),
            Some("dark") => Ok(Self::Dark),
            _ => Err(invalid(format!("`{at}.scheme` must be `light` or `dark`"))),
        }
    }
}

/// A validated CSS color, stored as the CSS text it emits.
///
/// As with [`FamilyList`], the text is constructed, never copied: hex digits
/// are re-emitted lowercased behind their `#`, `transparent` is the one
/// keyword the grammar knows, and a function call is rebuilt from its
/// validated name and component tokens. A value that cannot be built this
/// way is a configuration error, so a declaration can never carry a `var()`,
/// a nested call, a `url()`, or a `;` that would end the rule early.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Color(String);

/// The color functions a theme may call: the modern spaces the palettes are
/// authored in plus the legacy forms authors still reach for. Closed,
/// because an open function set is an open grammar.
const COLOR_FUNCTIONS: &[&str] = &["rgb", "rgba", "hsl", "hsla", "oklch", "oklab", "lab", "lch"];

/// The angle units a hue component may carry.
const HUE_UNITS: &[&str] = &["deg", "grad", "rad", "turn"];

impl Color {
    /// The CSS text, e.g. `#2e3440` or `oklch(0.7 0.1 250 / 50%)`.
    fn as_css(&self) -> &str {
        &self.0
    }

    /// Reads a color value at configuration path `at`.
    fn read(value: &Value, at: &str) -> Result<Self, ConfigErrorKind> {
        let raw = value
            .as_str()
            .ok_or_else(|| invalid(format!("`{at}` must be a string")))?;
        Self::parse(raw, at)
    }

    /// Parses one color: a hex triplet or quad, `transparent`, or a call
    /// from [`COLOR_FUNCTIONS`].
    fn parse(raw: &str, at: &str) -> Result<Self, ConfigErrorKind> {
        let raw = raw.trim();
        if let Some(hex) = raw.strip_prefix('#') {
            if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid(format!(
                    "`{at}`: {raw:?} is not a hex color; write #rgb, #rgba, \
                     #rrggbb, or #rrggbbaa"
                )));
            }
            let mut css = String::with_capacity(hex.len() + 1);
            css.push('#');
            css.extend(hex.chars().map(|c| c.to_ascii_lowercase()));
            return Ok(Self(css));
        }
        if let Some(open) = raw.find('(') {
            return Self::parse_function(raw, open, at);
        }
        if raw.eq_ignore_ascii_case("transparent") {
            return Ok(Self("transparent".to_string()));
        }
        Err(invalid(format!(
            "`{at}`: {raw:?} is not a color this grammar knows; write a hex \
             color (#rgb, #rrggbb), `transparent`, or a function call such as \
             `oklch(0.7 0.1 250)` or `rgb(46, 52, 64)`"
        )))
    }

    /// Parses `name(components)`: a closed function set over `none`,
    /// numbers, percentages, and angles, re-emitted from the tokens rather
    /// than copied. Nesting, stray bytes, and a body that does not end at
    /// its `)` are parse errors — the point is that no other shape can be
    /// expressed, not that dangerous characters are stripped.
    fn parse_function(raw: &str, open: usize, at: &str) -> Result<Self, ConfigErrorKind> {
        let Some(name) = COLOR_FUNCTIONS
            .iter()
            .find(|name| raw[..open].eq_ignore_ascii_case(name))
        else {
            return Err(invalid(format!(
                "`{at}`: {raw:?} calls `{}`; the color functions are: {}",
                &raw[..open],
                COLOR_FUNCTIONS.join(", ")
            )));
        };
        let Some(body) = raw[open + 1..].strip_suffix(')') else {
            return Err(invalid(format!(
                "`{at}`: {raw:?} does not end at its closing `)`"
            )));
        };
        if body.contains(['(', ')']) {
            return Err(invalid(format!(
                "`{at}`: {raw:?} nests parentheses; color components are \
                 numbers, percentages, angles, and `none`"
            )));
        }

        // Tokenize on spaces, commas, and slashes, remembering which
        // separators the author used: commas mean the legacy form, a slash
        // introduces the alpha component of the modern one.
        let mut tokens: Vec<(&str, bool)> = Vec::new();
        let mut start: Option<usize> = None;
        let mut after_slash = false;
        let mut filled = false;
        let mut commas = false;
        let mut empty_argument = false;
        for (index, c) in body.char_indices() {
            match c {
                ' ' | ',' | '/' => {
                    if let Some(from) = start.take() {
                        tokens.push((&body[from..index], after_slash));
                        after_slash = false;
                        filled = true;
                    }
                    if c != ' ' {
                        empty_argument |= !filled;
                        commas |= c == ',';
                        after_slash = c == '/';
                        filled = false;
                    }
                }
                _ => {
                    if start.is_none() {
                        start = Some(index);
                    }
                }
            }
        }
        if let Some(from) = start {
            tokens.push((&body[from..], after_slash));
            filled = true;
        }
        if tokens.is_empty() {
            return Err(invalid(format!("`{at}`: {raw:?} has no components")));
        }
        if empty_argument || !filled {
            return Err(invalid(format!(
                "`{at}`: {raw:?} has an empty component (check its commas and \
                 slashes)"
            )));
        }

        let mut css = String::with_capacity(raw.len() + 4);
        css.push_str(name);
        css.push('(');
        for (index, (token, after_slash)) in tokens.iter().enumerate() {
            if !color_component(token) {
                return Err(invalid(format!(
                    "`{at}`: {raw:?} has the component {token:?}; components \
                     are `none`, numbers, percentages, and angles ({})",
                    HUE_UNITS.join(", ")
                )));
            }
            if index > 0 {
                css.push_str(if commas {
                    ", "
                } else if *after_slash {
                    " / "
                } else {
                    " "
                });
            }
            css.extend(token.chars().map(|c| c.to_ascii_lowercase()));
        }
        css.push(')');
        Ok(Self(css))
    }
}

/// Validates a theme id: `[a-z][a-z0-9-]{0,31}`, hand-rolled because the
/// charset *is* the safety property. An id reaches CSS inside
/// `[data-theme="…"]` and the boot script inside a string literal, and no
/// character here can close either — `../`, a quote, and a brace are parse
/// errors, the same way a font file name's separator is.
fn theme_id<'a>(id: &'a str, at: &str) -> Result<&'a str, ConfigErrorKind> {
    let shape = || {
        invalid(format!(
            "`{at}`: theme id {id:?} must match `[a-z][a-z0-9-]{{0,31}}` — a \
             lowercase letter, then up to 31 more lowercase letters, digits, \
             or `-`"
        ))
    };
    let mut chars = id.chars();
    if !matches!(chars.next(), Some('a'..='z')) {
        return Err(shape());
    }
    if id.len() > 32 || !chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-')) {
        return Err(shape());
    }
    Ok(id)
}

/// Whether a token is a color component: `none`, a number, a percentage, or
/// an angle. Everything else — a `var(`, a quote, a semicolon, a stray
/// letter — falls out here.
fn color_component(token: &str) -> bool {
    if token.eq_ignore_ascii_case("none") {
        return true;
    }
    let split = token
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(token.len());
    let (number, unit) = token.split_at(split);
    let unit_known = unit.is_empty()
        || unit == "%"
        || HUE_UNITS.iter().any(|hue| unit.eq_ignore_ascii_case(hue));
    unit_known && is_css_number(number)
}

/// Whether `text` is a CSS number: an optional sign, then digits with at
/// most one decimal point and at least one digit. The exponent form is left
/// out on purpose — `1e5` is a number CSS accepts and no palette needs, and
/// excluding it keeps the scanner a single pass over two character classes.
fn is_css_number(text: &str) -> bool {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    let ascii_digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    match digits.split_once('.') {
        Some((whole, fraction)) => {
            !fraction.is_empty() && ascii_digits(whole) && ascii_digits(fraction)
        }
        None => !digits.is_empty() && ascii_digits(digits),
    }
}

/// Reads an optional `weight` key: a CSS numeric weight, `1` to `1000`.
///
/// Numbers written as YAML strings are accepted — the value is re-emitted
/// from the parsed integer either way, so the quoting style is the author's
/// business, not the stylesheet's.
fn read_weight(value: Option<&Value>, at: &str) -> Result<Option<u16>, ConfigErrorKind> {
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    let weight = match value {
        Value::Int(n) => u16::try_from(*n).ok(),
        Value::String(s) => s.trim().parse::<u16>().ok(),
        _ => None,
    };
    match weight {
        Some(weight) if (1..=1000).contains(&weight) => Ok(Some(weight)),
        _ => Err(invalid(format!(
            "`{at}.weight` must be a whole number between 1 and 1000"
        ))),
    }
}

/// Validates a family name: the conservative charset that survives quoting
/// with nothing left over — letters and digits in any script, spaces, and
/// `-`, `_`, `.`, `+`. Quotes, backslashes, braces, and semicolons are the
/// bytes that could escape a CSS string, and none of them is here.
fn family_name<'a>(name: &'a str, at: &str) -> Result<&'a str, ConfigErrorKind> {
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '+')))
    {
        return Err(invalid(format!(
            "`{at}`: family name {name:?} contains {bad:?}; \
             names may use letters, digits, spaces, `-`, `_`, `.`, and `+`"
        )));
    }
    Ok(name)
}

/// Validates a font file name: flat, ASCII, and extension-checked.
///
/// Path separators, `..`, and leading dots are outside the charset, so
/// traversal is a configuration error here rather than a runtime check at
/// copy time.
fn font_file_name(name: &str, at: &str) -> Result<(String, FontFormat), ConfigErrorKind> {
    let reject = |why: &str| {
        Err(invalid(format!(
            "`{at}`: {name:?} {why}; use a flat file name under `{FONTS_DIR}`, \
             e.g. `newsreader-400.woff2`"
        )))
    };
    if name.is_empty() {
        return reject("is empty");
    }
    if name.starts_with('.') {
        return reject("starts with a dot");
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
    {
        return Err(invalid(format!(
            "`{at}`: {name:?} contains {bad:?}; file names may use ASCII \
             letters, digits, `.`, `-`, and `_`"
        )));
    }
    if name.contains("..") {
        return reject("contains `..`");
    }
    let Some((_, ext)) = name.rsplit_once('.') else {
        return reject("has no file extension");
    };
    let Some(format) = FontFormat::from_extension(ext) else {
        return Err(invalid(format!(
            "`{at}`: {name:?} is not a font format `okf site` serves; \
             use .woff2, .woff, .ttf, or .otf"
        )));
    };
    Ok((name.to_string(), format))
}

/// Reads a required string key of a mapping at configuration path `at`.
fn required_str<'a>(map: &'a Mapping, key: &str, at: &str) -> Result<&'a str, ConfigErrorKind> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("`{at}` needs a string `{key}`")))
}

/// A mapping-valued section: `Ok(None)` for a null (written but empty) one,
/// an error for any other type.
fn mapping_or_empty<'a>(
    value: &'a Value,
    at: &str,
) -> Result<Option<&'a Mapping>, ConfigErrorKind> {
    match value {
        Value::Null => Ok(None),
        Value::Mapping(map) => Ok(Some(map)),
        _ => Err(invalid(format!("`{at}` must be a mapping"))),
    }
}

/// Rejects any key a section does not define, naming the ones it does.
fn check_keys(map: &Mapping, at: &str, known: &[&str]) -> Result<(), ConfigErrorKind> {
    if map.iter().any(|(key, _)| key.as_str().is_none()) {
        return Err(invalid(format!("`{at}` keys must be strings")));
    }
    for key in map.keys() {
        if !known.contains(&key) {
            return Err(invalid(format!(
                "unknown key `{key}` in `{at}` (known keys: {})",
                known.join(", ")
            )));
        }
    }
    Ok(())
}

/// A shape/value complaint about otherwise well-formed YAML.
fn invalid(message: impl Into<String>) -> ConfigErrorKind {
    ConfigErrorKind::Invalid(message.into())
}

/// Why a bundle's `.okf/config.yaml` could not be used.
#[derive(Debug)]
pub struct ConfigError {
    path: PathBuf,
    kind: ConfigErrorKind,
}

impl ConfigError {
    /// A configuration error for the bundle at `root`, for problems found
    /// after parsing — a face naming a file that is not in [`FONTS_DIR`],
    /// say.
    #[must_use]
    pub fn invalid(root: &Path, message: impl Into<String>) -> Self {
        Self::new(config_path(root), invalid(message))
    }

    /// The configuration file the problem was found in.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What is wrong with it.
    #[must_use]
    pub const fn kind(&self) -> &ConfigErrorKind {
        &self.kind
    }

    const fn new(path: PathBuf, kind: ConfigErrorKind) -> Self {
        Self { path, kind }
    }
}

/// The kinds of configuration failure, mirroring the strictness rules.
#[derive(Debug)]
pub enum ConfigErrorKind {
    /// The file exists but could not be read.
    Read(io::Error),
    /// The file is not YAML the OKF subset parser accepts; the error carries
    /// the source line.
    Yaml(YamlError),
    /// The YAML parsed, but a key, type, or value is not one `okf site`
    /// knows.
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.kind)
    }
}

impl fmt::Display for ConfigErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => write!(f, "{e}"),
            Self::Yaml(e) => write!(f, "{e}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ConfigErrorKind::Read(e) => Some(e),
            ConfigErrorKind::Yaml(e) => Some(e),
            ConfigErrorKind::Invalid(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses config text, expecting success.
    fn config(text: &str) -> SiteConfig {
        SiteConfig::parse(text).expect("config parses")
    }

    /// Parses config text, expecting the rendered error message.
    fn error(text: &str) -> String {
        SiteConfig::parse(text)
            .expect_err("config is rejected")
            .to_string()
    }

    #[test]
    fn a_missing_file_is_the_default_config() {
        let root = std::env::temp_dir().join(format!("okf-web-cfg-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        assert_eq!(SiteConfig::load(&root).unwrap(), SiteConfig::default());
        assert!(SiteConfig::default().fonts.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_reads_the_dot_okf_file() {
        let root = std::env::temp_dir().join(format!("okf-web-cfg-load-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".okf")).unwrap();
        fs::write(config_path(&root), "site:\n  title: Travel knowledge\n").unwrap();
        let loaded = SiteConfig::load(&root).unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Travel knowledge"));

        // A malformed file errors with the file path and the YAML line.
        fs::write(config_path(&root), "site:\n  title: [unclosed\n").unwrap();
        let err = SiteConfig::load(&root).unwrap_err().to_string();
        assert!(err.contains(".okf/config.yaml"), "path in error: {err}");
        assert!(err.contains("line 2"), "yaml line in error: {err}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_and_sectionless_files_configure_nothing() {
        assert_eq!(config(""), SiteConfig::default());
        assert_eq!(config("# just a comment\n"), SiteConfig::default());
        assert_eq!(config("site:\n"), SiteConfig::default());
        assert_eq!(config("site:\n  fonts:\n"), SiteConfig::default());
    }

    #[test]
    fn unknown_sections_are_noted_unknown_keys_are_rejected() {
        let config = config("studio:\n  theme: dark\nsite:\n  title: Docs\n");
        assert_eq!(config.title.as_deref(), Some("Docs"));
        assert_eq!(config.notes.len(), 1);
        assert!(
            config.notes[0].contains("unknown section `studio`"),
            "note names the section: {:?}",
            config.notes
        );

        let err = error("site:\n  titel: Docs\n");
        assert!(err.contains("unknown key `titel`"), "{err}");
        assert!(err.contains("known keys: title, fonts"), "{err}");
        let err = error("site:\n  fonts:\n    bodyy: serif\n");
        assert!(err.contains("known keys: body, code, files"), "{err}");
    }

    #[test]
    fn wrong_types_are_rejected_with_the_path() {
        assert!(error("site: nope\n").contains("`site` must be a mapping"));
        assert!(error("- a\n- b\n").contains("must be a mapping of sections"));
        assert!(error("site:\n  title: \"  \"\n").contains("must not be empty"));
        assert!(
            error("site:\n  fonts:\n    body: [serif]\n")
                .contains("`site.fonts.body` must be a string")
        );
        assert!(
            error("site:\n  fonts:\n    files: {}\n").contains("`site.fonts.files` must be a list")
        );
    }

    #[test]
    fn family_lists_quote_names_and_keep_generic_keywords_bare() {
        let config = config(
            "site:\n  fonts:\n    body: \"Newsreader, Georgia, serif\"\n    \
             code: \"JetBrains Mono , ui-monospace,monospace\"\n",
        );
        assert_eq!(
            config.fonts.body.as_ref().map(FamilyList::as_css),
            Some("\"Newsreader\", \"Georgia\", serif")
        );
        assert_eq!(
            config.fonts.code.as_ref().map(FamilyList::as_css),
            Some("\"JetBrains Mono\", ui-monospace, monospace")
        );
        assert!(!config.fonts.is_empty());
    }

    #[test]
    fn family_names_cannot_carry_css() {
        // Straight at the validator: a YAML file cannot even carry some of
        // these (a bare `"` ends a quoted scalar), and the invariant under
        // test is "no configured byte reaches the stylesheet", not "YAML
        // rejects it first".
        for raw in [
            "Evil\"; } :root { color: red } .x {",
            "Evil'",
            "url(http://x)",
            "Font\\65",
            "A; B",
            "A(B)",
            "A/*x*/B",
            "A\nB",
            "A\tB",
        ] {
            let err = FamilyList::parse(raw, "site.fonts.body")
                .expect_err("family name is rejected")
                .to_string();
            assert!(
                err.contains("`site.fonts.body`"),
                "rejected with the path: {err}"
            );
        }
        // And through the file, for the shapes YAML does allow.
        assert!(error("site:\n  fonts:\n    body: \"Ev!l\"\n").contains("contains '!'"));
        assert!(error("site:\n  fonts:\n    body: \"Serif,,Georgia\"\n").contains("empty family"));
        assert!(error("site:\n  fonts:\n    body: \" \"\n").contains("empty family"));
    }

    #[test]
    fn font_files_validate_names_weights_and_styles() {
        let config = config(
            "site:\n  fonts:\n    files:\n      \
             - family: Newsreader\n        file: newsreader-400.woff2\n        weight: 400\n      \
             - family: Newsreader\n        file: nr-700i.ttf\n        weight: \"700\"\n        style: italic\n",
        );
        let files = &config.fonts.files;
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].family(), "Newsreader");
        assert_eq!(files[0].file(), "newsreader-400.woff2");
        assert_eq!(files[0].format(), FontFormat::Woff2);
        assert_eq!(files[0].weight(), Some(400));
        assert_eq!(files[0].style(), FontStyle::Normal);
        assert_eq!(files[1].format(), FontFormat::Ttf);
        assert_eq!(files[1].weight(), Some(700));
        assert_eq!(files[1].style(), FontStyle::Italic);

        let bad = |file: &str| {
            error(&format!(
                "site:\n  fonts:\n    files:\n      - family: F\n        file: {file}\n"
            ))
        };
        // Traversal shapes never reach the filesystem: the leading dot and
        // the separator are both outside the name charset.
        assert!(bad("\"../../etc/passwd\"").contains("starts with a dot"));
        assert!(bad("\"a/b.woff2\"").contains("contains '/'"));
        assert!(bad("\"sub\\\\b.woff2\"").contains("contains '\\\\'"));
        assert!(bad("\"..woff2\"").contains("starts with a dot"));
        assert!(bad("\"a..b.woff2\"").contains("contains `..`"));
        assert!(bad("\"font.css\"").contains("not a font format"));
        assert!(bad("\"font\"").contains("no file extension"));
        assert!(
            error("site:\n  fonts:\n    files:\n      - family: F\n        file: f.woff2\n        weight: 0\n")
                .contains("between 1 and 1000")
        );
        assert!(
            error("site:\n  fonts:\n    files:\n      - family: F\n        file: f.woff2\n        style: slanted\n")
                .contains("normal, italic, oblique")
        );
        assert!(
            error("site:\n  fonts:\n    files:\n      - file: f.woff2\n")
                .contains("needs a string `family`")
        );
        assert!(
            error("site:\n  fonts:\n    files:\n      - family: F\n")
                .contains("needs a string `file`")
        );
        assert!(error("site:\n  fonts:\n    files:\n      - nope\n").contains("must be a mapping"));
    }

    #[test]
    fn emitted_css_is_font_face_plus_token_override() {
        let config = config(
            "site:\n  fonts:\n    body: Newsreader, serif\n    files:\n      \
             - family: Newsreader\n        file: nr-400.woff2\n        weight: 400\n      \
             - family: Newsreader\n        file: nr-700i.otf\n        weight: 700\n        style: italic\n",
        );
        let css = config.fonts.to_css();
        assert!(css.starts_with("/* Generated by `okf site`"), "{css}");
        assert!(
            css.contains(
                "@font-face {\n  font-family: \"Newsreader\";\n  \
                 src: url(\"fonts/nr-400.woff2\") format(\"woff2\");\n  \
                 font-style: normal;\n  font-weight: 400;\n  font-display: swap;\n}\n"
            ),
            "{css}"
        );
        assert!(
            css.contains("src: url(\"fonts/nr-700i.otf\") format(\"opentype\");"),
            "{css}"
        );
        assert!(css.contains("font-style: italic;"), "{css}");
        assert!(
            css.contains(":root {\n  --font-body: \"Newsreader\", serif;\n}\n"),
            "{css}"
        );
        // No --font-code override when only `body` is configured.
        assert!(!css.contains("--font-code"), "{css}");
    }

    #[test]
    fn faces_alone_emit_no_token_override() {
        let config =
            config("site:\n  fonts:\n    files:\n      - family: F\n        file: f.woff\n");
        let css = config.fonts.to_css();
        assert!(css.contains("format(\"woff\")"), "{css}");
        assert!(!css.contains(":root"), "{css}");
        // A face with no weight descriptor leaves the CSS default in place.
        assert!(!css.contains("font-weight"), "{css}");
    }

    #[test]
    fn themes_emit_one_canonical_block_each() {
        let config = config(
            r##"site:
  themes:
    - id: nord
      label: "Nord"
      scheme: dark
      colors:
        ink: "#ECEFF4"
        surface: "#2e3440"
    - id: paper
      scheme: light
      colors:
        surface: "oklch(0.98 0.01 90 / 50%)"
"##,
        );
        assert_eq!(config.themes.len(), 2);
        let themes: Vec<_> = config.themes.iter().collect();
        assert_eq!(themes[0].id(), "nord");
        assert_eq!(themes[0].label(), "Nord");
        assert_eq!(themes[0].scheme(), Scheme::Dark);
        assert_eq!(themes[1].label(), "paper");
        assert_eq!(themes[1].scheme().as_css(), "light");
        assert_eq!(config.settings(), ["themes"]);
        assert_eq!(
            config.themes.to_css(),
            concat!(
                "/* Generated by `okf site` from `.okf/config.yaml`. Do not edit:\n",
                "   rebuild the site instead. Linked after the inline stylesheet, so\n",
                "   these token overrides win the cascade. */\n",
                "\n",
                ":root[data-theme=\"nord\"] {\n",
                "  --surface: #2e3440;\n",
                "  --ink: #eceff4;\n",
                "}\n",
                "\n",
                ":root[data-theme=\"paper\"] {\n",
                "  --surface: oklch(0.98 0.01 90 / 50%);\n",
                "}\n",
            )
        );
    }

    #[test]
    fn declarations_follow_the_token_order_not_the_authors() {
        let config = config(
            r##"site:
  themes:
    - id: nord
      scheme: dark
      colors:
        danger: "#bf616a"
        ink: "#eceff4"
        surface: "#2e3440"
"##,
        );
        assert!(
            config.themes.to_css().contains(
                ":root[data-theme=\"nord\"] {\n  --surface: #2e3440;\n  \
                 --ink: #eceff4;\n  --danger: #bf616a;\n}\n"
            ),
            "{}",
            config.themes.to_css()
        );
    }

    #[test]
    fn a_theme_may_declare_only_a_scheme() {
        for text in [
            "site:\n  themes:\n    - id: nord\n      scheme: dark\n",
            "site:\n  themes:\n    - id: nord\n      scheme: dark\n      colors:\n",
        ] {
            let config = config(text);
            let theme = config.themes.iter().next().expect("one theme");
            assert_eq!(theme.id(), "nord");
            // The label falls back to the id, which is what the picker shows.
            assert_eq!(theme.label(), "nord");
            assert_eq!(theme.scheme(), Scheme::Dark);
            assert!(!config.themes.is_empty());
            // A theme that overrides nothing is the base palette its scheme
            // already selects, so it contributes no block.
            let css = config.themes.to_css();
            assert!(css.starts_with("/* Generated by `okf site`"), "{css}");
            assert!(!css.contains("data-theme"), "{css}");
        }
        assert!(Themes::default().is_empty());
        assert_eq!(Themes::default().len(), 0);
    }

    #[test]
    fn theme_keys_and_shapes_are_checked() {
        assert!(error("site:\n  themes: {}\n").contains("`site.themes` must be a list"));
        assert!(
            error("site:\n  themes:\n    - nope\n")
                .contains("`site.themes[0]` must be a mapping with `id` and `scheme`")
        );
        let err = error("site:\n  themes:\n    - id: nord\n      scheme: dark\n      colour: x\n");
        assert!(
            err.contains("unknown key `colour` in `site.themes[0]`"),
            "{err}"
        );
        assert!(
            err.contains("known keys: id, label, scheme, colors"),
            "{err}"
        );
        let err = error(
            "site:\n  themes:\n    - id: nord\n      scheme: dark\n      \
             colors:\n        surfase: \"#000\"\n",
        );
        assert!(
            err.contains("unknown key `surfase` in `site.themes[0].colors`"),
            "{err}"
        );
        assert!(err.contains("surface, ink, edge"), "{err}");
        assert!(
            error("site:\n  themes:\n    - scheme: dark\n")
                .contains("`site.themes[0]` needs a string `id`")
        );
        assert!(
            error("site:\n  themes:\n    - id: nord\n")
                .contains("`site.themes[0]` needs a `scheme` of `light` or `dark`")
        );
        assert!(
            error("site:\n  themes:\n    - id: nord\n      scheme: blue\n")
                .contains("`site.themes[0].scheme` must be `light` or `dark`")
        );
        assert!(
            error("site:\n  themes:\n    - id: nord\n      scheme: dark\n      label: \"  \"\n")
                .contains("`site.themes[0].label` must not be empty")
        );
    }

    #[test]
    fn theme_ids_are_flat_and_lowercase() {
        let bad = |id: &str| {
            error(&format!(
                "site:\n  themes:\n    - id: {id}\n      scheme: dark\n"
            ))
        };
        // Nothing in the charset can close `[data-theme="…"]`, walk a path,
        // or end a declaration.
        for id in [
            "Nord",
            "1nord",
            "\"../etc\"",
            "\"nord theme\"",
            "\"nord\\\"]\"",
            "\"nord;color:red\"",
            "-nord",
            &"n".repeat(33),
        ] {
            let err = bad(id);
            assert!(err.contains("must match `[a-z][a-z0-9-]{0,31}`"), "{err}");
            assert!(err.contains("`site.themes[0].id`"), "{err}");
        }
        // The boundary length is accepted, so the rejection above is the
        // length rule and not the charset one.
        assert!(
            config(&format!(
                "site:\n  themes:\n    - id: {}\n      scheme: dark\n",
                "n".repeat(32)
            ))
            .themes
            .iter()
            .next()
            .is_some()
        );
        let err = bad("auto");
        assert!(err.contains("`auto` is reserved"), "{err}");

        let err = error(
            "site:\n  themes:\n    - id: nord\n      scheme: dark\n    \
             - id: nord\n      scheme: light\n",
        );
        assert!(err.contains("`site.themes[1]`"), "{err}");
        assert!(err.contains("duplicate theme id `nord`"), "{err}");
    }

    #[test]
    fn colors_are_re_emitted_from_the_parse() {
        let css = |raw: &str| {
            Color::parse(raw, "site.themes[0].colors.surface")
                .expect("color parses")
                .as_css()
                .to_string()
        };
        assert_eq!(css("#ABC"), "#abc");
        assert_eq!(css("#ABCD"), "#abcd");
        assert_eq!(css(" #2E3440 "), "#2e3440");
        assert_eq!(css("#2E344080"), "#2e344080");
        assert_eq!(css("transparent"), "transparent");
        assert_eq!(css("RGB(46,52 , 64)"), "rgb(46, 52, 64)");
        assert_eq!(css("rgba(46, 52, 64, .5)"), "rgba(46, 52, 64, .5)");
        assert_eq!(css("oklch(0.7 0.1 250)"), "oklch(0.7 0.1 250)");
        assert_eq!(
            css("OKLCH(70% 0.1 250DEG  /  50%)"),
            "oklch(70% 0.1 250deg / 50%)"
        );
        assert_eq!(css("oklch(0.7 NONE -0.5turn)"), "oklch(0.7 none -0.5turn)");
        assert_eq!(css("hsl(210 100% 50% / none)"), "hsl(210 100% 50% / none)");
        assert_eq!(css("lab(52.2% +40.1 59.9)"), "lab(52.2% +40.1 59.9)");
    }

    #[test]
    fn colors_cannot_carry_css() {
        // Straight at the grammar, as with family names: the invariant is
        // "no configured byte reaches the stylesheet", not "YAML rejects it".
        for raw in [
            "var(--x)",
            "url(x)",
            "red",
            "oklch(1 2 3);}",
            "rgb(var(--x))",
            "rgb(1 2 calc(3))",
            "oklch(1 2 3",
            "oklch(1 2 3) ; color: red",
            "#",
            "#12345",
            "#ghijkl",
            "rgb()",
            "rgb(1,,2)",
            "rgb(1, 2, 3,)",
            "rgb(1 2 red)",
            "rgb(1e5 2 3)",
            "hsl(210 100% 50%\")",
            "",
        ] {
            let err = Color::parse(raw, "site.themes[0].colors.surface")
                .expect_err("color is rejected")
                .to_string();
            assert!(
                err.contains("`site.themes[0].colors.surface`"),
                "rejected with the path: {err}"
            );
        }
        // And through the file, where the path is built by the reader.
        let err = error(
            "site:\n  themes:\n    - id: nord\n      scheme: dark\n      \
             colors:\n        ink: \"rgb(1, var(--x))\"\n",
        );
        assert!(err.contains("`site.themes[0].colors.ink`"), "{err}");
        assert!(
            error(
                "site:\n  themes:\n    - id: nord\n      scheme: dark\n      \
                 colors:\n        ink: [black]\n"
            )
            .contains("`site.themes[0].colors.ink` must be a string")
        );
    }

    #[test]
    fn the_default_theme_is_an_id_or_auto() {
        assert_eq!(
            config("site:\n  theme: auto\n").theme.as_deref(),
            Some("auto")
        );
        let config = config(
            "site:\n  title: Docs\n  theme: nord\n  themes:\n    \
             - id: nord\n      scheme: dark\n",
        );
        assert_eq!(config.theme.as_deref(), Some("nord"));
        // `theme` and `themes` report after the existing four settings.
        assert_eq!(config.settings(), ["title", "theme", "themes"]);
        assert!(error("site:\n  theme: Nord\n").contains("`site.theme`"));
        assert!(error("site:\n  theme: \"  \"\n").contains("`site.theme` must not be empty"));
        assert!(error("site:\n  theme: [nord]\n").contains("`site.theme` must be a string"));
    }
}
