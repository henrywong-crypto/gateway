use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use inference_profiles::{get_inference_profile_costs, get_inference_profiles};
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;

use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct CostParams {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

pub async fn view_inference_profile_costs(
    session: Session,
    state: State<AppState>,
    query: Query<CostParams>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let now = time::OffsetDateTime::now_utc();
    let start_date = query.start_date.clone().unwrap_or_else(|| {
        let first_of_month = now.replace_day(1).unwrap_or(now);
        format!(
            "{:04}-{:02}-{:02}",
            first_of_month.year(),
            first_of_month.month() as u8,
            first_of_month.day()
        )
    });
    let end_date = query.end_date.clone().unwrap_or_else(|| {
        format!(
            "{:04}-{:02}-{:02}",
            now.year(),
            now.month() as u8,
            now.day()
        )
    });

    let profiles = get_inference_profiles(&state.db_pool, &email).await?;

    let arns: Vec<String> = profiles
        .iter()
        .map(|p| p.inference_profile_arn.clone())
        .collect();

    let costs = get_inference_profile_costs(&arns, &start_date, &end_date)
        .await
        .unwrap_or_default();

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
                <td>${:.4}</td>
            </tr>"#,
            profile.inference_profile_name, profile.model_name, cost
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
                <h1>Inference Profile Costs for {email}</h1>
                <p>Period: {} to {}</p>
                <table>
                    <thead>
                        <tr>
                            <th>Profile Name</th>
                            <th>Model</th>
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
