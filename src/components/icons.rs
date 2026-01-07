//! Centralized icon definitions.
//!
//! Icon theme is configured in `config.rs` via `ICON_THEME`.
//! This module maps semantic icon names to the selected theme's icons.

use icondata::Icon;

use crate::config::IconTheme;

// =============================================================================
// Theme Imports
// =============================================================================

mod lucide {
    pub use icondata::{
        LuAArrowDown as FontDecrease, LuAArrowUp as FontIncrease, LuBookOpen as FilePdf,
        LuChevronLeft as ChevronLeft, LuChevronRight as ChevronRight, LuDownload as Download,
        LuEllipsisVertical as More, LuExternalLink as ExternalLink, LuFile as File,
        LuFileText as FileText, LuFolder as Folder, LuFolderOpen as Explorer, LuGlobe as Network,
        LuHouse as Home, LuImage as FileImage, LuLayoutGrid as Grid, LuLink as FileLink,
        LuList as List, LuLock as Lock, LuMapPin as Location, LuPencil as Edit, LuPlus as Plus,
        LuSearch as Search, LuShare2 as Share, LuTerminal as Terminal, LuUser as User,
        LuX as Close,
    };
}

mod bootstrap {
    pub use icondata::{
        BsBoxArrowUpRight as ExternalLink, BsChevronLeft as ChevronLeft,
        BsChevronRight as ChevronRight, BsDownload as Download, BsFileEarmark as File,
        BsFileEarmarkImage as FileImage, BsFileEarmarkPdf as FilePdf,
        BsFileEarmarkText as FileText, BsFolder2 as Explorer, BsFolderFill as Folder,
        BsGeoAltFill as Location, BsGlobe as Network, BsGrid as Grid, BsHouseFill as Home,
        BsLink45deg as FileLink, BsListUl as List, BsLockFill as Lock, BsPencil as Edit,
        BsPerson as User, BsPlusLg as Plus, BsSearch as Search, BsShare as Share,
        BsTerminal as Terminal, BsThreeDotsVertical as More, BsTypeBold as FontDecrease,
        BsTypeBold as FontIncrease, BsXLg as Close,
    };
}

// =============================================================================
// Icon Constants (selected based on theme)
// =============================================================================

macro_rules! themed_icon {
    ($name:ident, $theme_name:ident) => {
        pub const $name: Icon = match crate::config::ICON_THEME {
            IconTheme::Lucide => lucide::$theme_name,
            IconTheme::Bootstrap => bootstrap::$theme_name,
        };
    };
}

themed_icon!(CHEVRON_LEFT, ChevronLeft);
themed_icon!(CHEVRON_RIGHT, ChevronRight);
themed_icon!(HOME, Home);
themed_icon!(FOLDER, Folder);
themed_icon!(FILE, File);
themed_icon!(FILE_TEXT, FileText);
themed_icon!(FILE_PDF, FilePdf);
themed_icon!(FILE_IMAGE, FileImage);
themed_icon!(FILE_LINK, FileLink);
themed_icon!(SEARCH, Search);
themed_icon!(LIST, List);
themed_icon!(GRID, Grid);
themed_icon!(PLUS, Plus);
themed_icon!(MORE, More);
themed_icon!(TERMINAL, Terminal);
themed_icon!(EXPLORER, Explorer);
themed_icon!(LOCK, Lock);
themed_icon!(CLOSE, Close);
themed_icon!(EXTERNAL_LINK, ExternalLink);
themed_icon!(EDIT, Edit);
themed_icon!(FONT_INCREASE, FontIncrease);
themed_icon!(FONT_DECREASE, FontDecrease);
themed_icon!(SHARE, Share);
themed_icon!(DOWNLOAD, Download);
themed_icon!(USER, User);
themed_icon!(LOCATION, Location);
themed_icon!(NETWORK, Network);
