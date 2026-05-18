pub mod device {
    // use sea_orm::entity::prelude::*;
    use sea_orm::*;
    // use crate::entities::Group;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Default)]
    #[sea_orm(table_name = "devices")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = true)]
        pub id: i32,
        pub uuid: String,
        pub hostname: String,
        pub group_id: Option<i32>,
        // #[sea_orm(belongs_to, from = "group_id", to = "id")]
        // pub group: HasOne<crate::entities::group::group::Entity>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "crate::entities::group::group::Entity",
            from = "Column::GroupId",
            to = "crate::entities::group::group::Column::Id"
        )]
        Group,
    }

    impl Related<crate::entities::group::group::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Group.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
