use crate::core::constants::VERSION;
use stblib::colors::{BOLD, C_RESET, CYAN, GREEN, MAGENTA, RED, RESET, UNDERLINE, WHITE};

pub fn help() {
    println!(
        "\
{BOLD}{CYAN}{UNDERLINE}Rustle - Fast File Transfer Tool v{}{C_RESET}\n\
{GREEN}{BOLD}Usage:{RESET} {WHITE}rustle {CYAN}[command] {RED}[<options>]{C_RESET}\n\n\
{MAGENTA}{BOLD}Commands:{C_RESET}
    {CYAN}{BOLD}help:{C_RESET} Prints this message
    {CYAN}{BOLD}send <file>:{C_RESET} Send a file to discovered device
    {CYAN}{BOLD}daemon:{C_RESET} Start daemon (mDNS + file transfer server)
    {CYAN}{BOLD}scan:{C_RESET} Show all mDNS services
",
        *VERSION
    );
    std::process::exit(0);
}
