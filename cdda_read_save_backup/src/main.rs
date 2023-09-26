use cdda_save_backup::read_backup_save;
use std::{
    env::args,
    io::{self, Read},
    process::exit,
};
fn main() {
    let args = args().collect::<Vec<String>>();
    let mut buffer = [0; 1];
    let backup_save_path = args.get(1).unwrap_or_else(|| {
        println!("请输入存档备份文件的路径!");
        println!("按回车键退出");
        io::stdin().read_exact(&mut buffer).unwrap();
        exit(0)
    });
    let output_directory = args.get(2).to_owned().map(|x| x.as_str());
    println!("开始读取存档备份文件:{}", backup_save_path);
    match read_backup_save(backup_save_path, output_directory) {
        Ok(_) => println!("读取存档备份文件成功!"),
        Err(err) => {
            println!("读取存档备份文件失败:{}", err);
            println!("按回车键退出");
            io::stdin().read_exact(&mut buffer).unwrap();
            exit(1)
        }
    }
}
