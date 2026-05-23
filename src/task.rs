pub struct Task {
    pub id: i64,
    pub title: String,
    pub position: i64,
    pub done: bool,
    pub created_at: String,
    pub notes: String,
    pub tags: Vec<String>,
}
