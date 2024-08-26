use axum::{
    extract::{State},
    response::{IntoResponse, Redirect},
    Form,
};
use maud::html;
use serde::Deserialize;
use crate::AppState;

pub(crate) async fn get(State(_app_state): State<AppState>) -> impl IntoResponse {
    html! {
      form method="post" action="/_chainedge/auth" {
        input type="password" name="password";

        input type="submit" value="Login";
      }
    }
}

#[derive(Deserialize)]
pub(crate) struct FormState {
    password: String,
}

pub(crate) async fn post(
    State(state): State<AppState>,
    Form(form): Form<FormState>,
) -> impl IntoResponse {
    if form.password != state.admin_password {
        return Redirect::to("/_chainedge/list");
    }
    
    Redirect::to("/_chainedge/list")
}
