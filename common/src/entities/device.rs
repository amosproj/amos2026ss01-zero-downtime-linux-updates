pub mod device {
    use sea_orm::entity::prelude::*;
    use crate::entities::Group;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "devices")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = true)]
        pub id: i32,

        pub uuid: String,

        pub hostname: String,

        pub group_id: Option<i32>,
        #[sea_orm(belongs_to, from = "group_id", to = "id")]
        pub group: HasOne<Group::Entity>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
