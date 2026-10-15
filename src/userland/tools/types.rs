//! Engineering tool types.

#[cfg(feature = "bm-engineering-tools")]
use crate::platform::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandGroup {
    Shell,
    File,
    Text,
    Build,
    Process,
    System,
    Debug,
    Device,
    Network,
    Phase,
    Security,
    Benchmark,
    Script,
}

impl CommandGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::File => "file",
            Self::Text => "text",
            Self::Build => "build",
            Self::Process => "process",
            Self::System => "system",
            Self::Debug => "debug",
            Self::Device => "device",
            Self::Network => "network",
            Self::Phase => "phase",
            Self::Security => "security",
            Self::Benchmark => "benchmark",
            Self::Script => "script",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthorityRequirement {
    Public,
    Viewer,
    Operator,
    Root,
    AnomAdministrator,
}

impl AuthorityRequirement {
    pub fn label(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Root => "root",
            Self::AnomAdministrator => "anom-administrator",
        }
    }

    pub fn allows(self, have: AuthorityRequirement) -> bool {
        have >= self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSupport {
    Hosted,
    Baremetal,
    Both,
    Future,
}

impl TargetSupport {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hosted => "hosted",
            Self::Baremetal => "baremetal",
            Self::Both => "both",
            Self::Future => "future",
        }
    }

    pub fn available_on(self, target: RuntimeTarget) -> bool {
        match (self, target) {
            (Self::Both, _) => true,
            (Self::Hosted, RuntimeTarget::Hosted) => true,
            (Self::Baremetal, RuntimeTarget::Baremetal) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTarget {
    Hosted,
    Baremetal,
}

pub fn current_runtime_target() -> RuntimeTarget {
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        return RuntimeTarget::Baremetal;
    }
    #[cfg(feature = "std")]
    {
        return RuntimeTarget::Hosted;
    }
    #[cfg(not(any(feature = "std", all(feature = "baremetal", not(feature = "std")))))]
    {
        RuntimeTarget::Hosted
    }
}

#[cfg(feature = "bm-engineering-tools")]
pub type ToolHandler =
    fn(&mut super::context::CommandContext<'_>, &[String]) -> Result<i32, super::error::ToolError>;

#[cfg(feature = "bm-engineering-tools")]
pub struct CommandDescriptor {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub group: CommandGroup,
    pub usage: &'static str,
    pub description: &'static str,
    pub authority: AuthorityRequirement,
    pub target_support: TargetSupport,
    pub handler: ToolHandler,
}

#[cfg(feature = "bm-engineering-tools")]
pub fn full_command_name(desc: &CommandDescriptor) -> String {
    format!("{} {}", desc.group.label(), desc.name)
}
