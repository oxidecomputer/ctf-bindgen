use crate::ctf::{Ctf, TypeInfo, TypeRepr};

pub fn print(ctf_info: &Ctf, debug: bool) {
    if debug {
        println!("{:#?}", ctf_info.sections);
    }

    println!("FUNCTIONS");
    println!("=========");
    for (i, f) in ctf_info.sections.functions.iter().enumerate() {
        if f.types.is_empty() {
            continue;
        }
        let mut types = Vec::new();
        for t in &f.types {
            types.push(ctf_info.type_name(*t))
        }
        let ret = types[0];
        let args = if types.len() > 1 {
            format!("{:?}", &types[1..])
        } else {
            String::new()
        };
        let name = &ctf_info.function_names[i];
        println!("{} -> ({}) {}", name, args, ret,);
    }

    println!("TYPES");
    println!("=====");
    for t in &ctf_info.sections.types {
        match &t.repr {
            TypeRepr::Struct(members) => {
                println!("struct {}:", ctf_info.string_at(t.name_offset));

                for m in members {
                    let i = m.type_offset as usize - 1;
                    let typ = &ctf_info.sections.types[i];
                    let sz = match typ.info {
                        TypeInfo::Size(sz) => sz,
                        TypeInfo::Type(_) => todo!("typeinfo type"),
                    };
                    println!(
                        "  {} {} size={} offset={}",
                        ctf_info.string_at(m.name_offset),
                        ctf_info.string_at(typ.name_offset),
                        sz,
                        m.offset,
                    );
                }
            }
            TypeRepr::Int(_) => println!("int"),
            TypeRepr::Float(_) => println!("int"),
        }
    }
}
