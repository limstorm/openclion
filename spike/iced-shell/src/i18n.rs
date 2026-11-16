//! A slice of the web catalog. The real crate loads `src/platform/locales/*.json`; keeping the
//! lookup here means every string in the shell goes through one function from day one, which is
//! what makes the switch to the full catalog a data change instead of a code change.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Zh,
}

pub fn t(language: Language, key: &'static str) -> &'static str {
    match (language, key) {
        (_, "app.title") => "OpenCLion",
        (Language::En, "workbench.welcome") => "Welcome to OpenCLion",
        (Language::Zh, "workbench.welcome") => "欢迎使用 OpenCLion",
        (Language::En, "palette.placeholder") => "Type a command…",
        (Language::Zh, "palette.placeholder") => "输入命令…",
        (Language::En, "status.ready") => "ready",
        (Language::Zh, "status.ready") => "就绪",
        (Language::En, "tool.project") => "Project",
        (Language::Zh, "tool.project") => "项目",
        (Language::En, "tool.search") => "Search",
        (Language::Zh, "tool.search") => "搜索",
        (Language::En, "tool.git") => "Git",
        (Language::Zh, "tool.git") => "Git",
        (Language::En, "tool.problems") => "Problems",
        (Language::Zh, "tool.problems") => "问题",
        (Language::En, "tool.terminal") => "Terminal",
        (Language::Zh, "tool.terminal") => "终端",
        (Language::En, "tool.run") => "Run",
        (Language::Zh, "tool.run") => "运行",
        (Language::En, "tool.structure") => "Structure",
        (Language::Zh, "tool.structure") => "结构",
        (Language::En, "tool.commit") => "Commit",
        (Language::Zh, "tool.commit") => "提交",
        (Language::En, "tool.find") => "Find",
        (Language::Zh, "tool.find") => "查找",
        (Language::En, "tool.usages") => "Find Usages",
        (Language::Zh, "tool.usages") => "查找用法",
        (Language::En, "tool.todo") => "TODO",
        (Language::Zh, "tool.todo") => "TODO",
        (Language::En, "tool.cmake") => "CMake",
        (Language::Zh, "tool.cmake") => "CMake",
        (Language::En, "tool.build") => "Build",
        (Language::Zh, "tool.build") => "构建",
        (Language::En, "tool.database") => "Database",
        (Language::Zh, "tool.database") => "数据库",
        (_, other) => other,
    }
}
