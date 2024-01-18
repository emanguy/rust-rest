create table todo_user (
    id serial primary key not null,
    first_name varchar(128) not null,
    last_name varchar(128) not null
);

create table todo_item (
    id serial primary key not null,
    user_id integer not null,
    item_desc text not null,
    test text not null,

    constraint todo_item_user_id_fk foreign key(user_id) references todo_user(id)
);
