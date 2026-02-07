use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use inference_profiles::{get_inference_profile_costs, get_inference_profiles};
use myerrors::AppError;
use myhandlers::AppState;
use time::OffsetDateTime;
use tower_sessions::Session;

use crate::templates::common::{common_styles, nav_menu};

pub async fn inference_profile_costs_get(
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let profiles = get_inference_profiles(&state.db_pool, &email).await?;

    let arns: Vec<String> = profiles
        .iter()
        .map(|p| p.inference_profile_arn.clone())
        .collect();

    let now = OffsetDateTime::now_utc();
    let start_date = format!(
        "{}-{:02}-01",
        now.year(),
        now.month() as u8
    );
    let end_date = format!(
        "{}-{:02}-{:02}",
        now.year(),
        now.month() as u8,
        now.day()
    );

    let costs = get_inference_profile_costs(&arns, &start_date, &end_date).await?;

    let mut rows = String::new();
    for profile in &profiles {
        let cost = costs
            .iter()
            .find(|(arn, _)| arn == &profile.inference_profile_arn)
            .map(|(_, c)| *c)
            .unwrap_or(0.0);

        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td>${:.2}</td>
            </tr>"#,
            profile.model_name, profile.inference_profile_name, cost
        ));
    }

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Inference Profile Costs</h1>
                <p>Current month: {} to {}</p>
                <table>
                    <thead>
                        <tr>
                            <th>Model</th>
                            <th>Inference Profile Name</th>
                            <th>Cost</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows}
                    </tbody>
                </table>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        start_date,
        end_date,
        nav_menu()
    );

    Ok(Html(html).into_response())
}
