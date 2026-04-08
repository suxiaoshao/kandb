use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionFormSchema {
    pub title: String,
    pub fields: Vec<FormField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormField {
    pub key: String,
    pub label: String,
    pub kind: FormFieldKind,
    pub required: bool,
    pub help_text: Option<String>,
    pub placeholder: Option<String>,
    #[serde(default)]
    pub options: Vec<FormSelectOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFieldKind {
    Text,
    Secret,
    Checkbox,
    Select,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormSelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarTree {
    pub roots: Vec<TreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub icon: IconToken,
    pub children: TreeChildren,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeChildren {
    Leaf,
    Branch(Vec<TreeNode>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconToken {
    Database,
    Folder,
    HardDrive,
    Table,
    View,
    Column,
    Key,
    Index,
}
