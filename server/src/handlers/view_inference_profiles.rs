use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use inference_profiles::get_inference_profiles;
use myerrors::AppError;
use myhandlers::AppState;
use tower_sessions::Session;

use crate::templates::common::{common_styles, nav_menu};

pub async fn view_inference_profiles(
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let profiles = get_inference_profiles(&state.db_pool, &email).await?;

    let mut rows = String::new();
    for profile in profiles {
        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            profile.inference_profile_name,
            profile.model_arn,
            profile.inference_profile_arn,
            profile.created_at
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
                <h1>Inference Profiles for {email}</h1>
                <table>
                    <thead>
                        <tr>
                            <th>Profile Name</th>
                            <th>Model</th>
                            <th>ARN</th>
                            <th>Created</th>
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
        nav_menu()
    );

    Ok(Html(html).into_response())
}
