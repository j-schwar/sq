mod arena;
mod schema;
mod sql;
mod syntax;

/*
sel test q 'report code=visit.edit'
sel test q '(report code=visit.edit)>param'
sel test q 'report>param code=visit.edit'
sel test q 'report>param+data code=visit.edit'
sel test q 'report>data(data.source) code=visit.edit data.code=execute'
sel test q 'parameters report.code=std.pr.hm.visit.edit'
sel test q 'datasource(source) report.code=std.pr.hm.visit.edit code=execute'
*/

fn main() {
    println!("Hello, world!");
}
