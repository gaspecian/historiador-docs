use uuid::Uuid;

pub struct CreateCollectionCommand {
    pub name: String,
    pub parent_id: Option<Uuid>,
}

pub struct UpdateCollectionCommand {
    pub id: Uuid,
    pub name: Option<String>,
    /// `Some(None)` → move to root; `None` → leave unchanged.
    pub parent_id: Option<Option<Uuid>>,
}
