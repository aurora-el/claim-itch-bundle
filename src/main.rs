use std::process::Stdio;
use clap::{Parser, ValueEnum};
use fantoccini::{ClientBuilder, Locator};
use serde_json::json;
use tokio::process;

/// A program to automate claiming itch.io games purchased as part of large bundles
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    /// itchio username
    #[arg(short, long)]
    username: String,

    /// itchio password
    #[arg(short, long)]
    password: String,

    /// bundle name
    #[arg(short, long, required=true)]
    bundle: Vec<String>,

    /// select webdriver to use (should be installed on your system and in PATH)
    #[arg(short, long, default_value="chrome")]
    webdriver: WebDriver,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum WebDriver {
    /// use chromedriver
    Chrome,
    
    /// use geckodriver
    Firefox
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse CLI args
    let args = Args::parse();

    // start webdriver in child process
    let mut driver = process::Command::new(match args.webdriver {
        WebDriver::Chrome => "chromedriver",
        WebDriver::Firefox => "geckodriver",
    })
        .arg("--port=4444").kill_on_drop(true).stdout(Stdio::null())
        .spawn().expect("Failed to start WebDriver, perhaps it isn't installed?");

    // start headless client and connect to webdriver
    let client = ClientBuilder::native()
        .capabilities(serde_json::Map::from_iter([
            ("moz:firefoxOptions".into(), json!({
                "args": ["-headless"]
            })),
            ("goog:chromeOptions".into(), json!({
                "args": ["--headless"]
            })),
        ]))
        .connect("http://localhost:4444").await?;

    
    // login to itch.io using supplied credentials
    client.goto("https://itch.io/login").await?;
    client.form(Locator::Css(".login_form_widget > form")).await?
        .set_by_name("username", &args.username).await?
        .set_by_name("password", &args.password).await?
        .submit_direct().await?;
    println!("Logging in to itch.io as {:?}", &args.username);
    
    // wait for login to complete
    client.wait().for_element(Locator::Css("[data-page_name='dashboard']")).await?;


    for bundle in &args.bundle {
        client.goto("https://itch.io/my-purchases/bundles").await?;
        println!("Claiming bundle: {:?}", &bundle);
        client.find(Locator::LinkText(&bundle)).await?.click().await?;

        loop {
            // log current page number
            println!("{}", client.find(Locator::Css(".pager_label")).await?.text().await?);
            
            // claim first unclaimed game on page
            while let Ok(game) = client.find(Locator::Css("button[value=claim]")).await {
                game.click().await?;
                
                // log the game's name
                println!("{:?} claimed",
                    client
                        .find(Locator::Css(".game_download_header_widget .object_title")).await?
                        .text().await?);
            
                client.back().await?;
            }

            // go to next page if there is one
            if let Ok(next_page) = client.find(Locator::Css(".button.next_page")).await {
                next_page.click().await?;
            } else {
                break;
            }
        }
        
    }

    // clean up
    client.close().await?;
    driver.kill().await?;

    Ok(())

}
