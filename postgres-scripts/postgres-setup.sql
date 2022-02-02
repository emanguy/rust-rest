create table user (
    id integer primary key,
    first_name varchar(128) not null,
    last_name varchar(128) not null,
);

create table todo_item (
    id integer primary key,
    user_id integer not null,
    item_desc text not null,

    constraint todo_item_user_id_fk foreign key(user_id) references user(id),
);