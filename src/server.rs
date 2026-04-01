use rmcp::{
    ErrorData as McpError, ServerHandler, handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
};
use serde_json::Value;
use tracing::debug;

use crate::api::FreshBooksClient;
use crate::params::{
    CreateClientParams, CreateExpenseParams, CreateInvoiceParams, ExchangeCodeParams, GetByIdParams,
    ListExpensesParams, ListParams, ListPaymentsParams, SearchParams, UpdateClientParams,
};

#[derive(Clone)]
pub struct FreshBooksServer {
    client: std::sync::Arc<FreshBooksClient>,
    tool_router: ToolRouter<Self>,
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn fmt_client_row(c: &Value) -> String {
    let id = c["id"].as_u64().unwrap_or(0);
    let fname = c["fname"].as_str().unwrap_or("");
    let lname = c["lname"].as_str().unwrap_or("");
    let email = c["email"].as_str().unwrap_or("");
    let org = c["organization"].as_str().unwrap_or("");
    format!("{id:<8} {fname} {lname:<20} {email:<32} {org}")
}

fn fmt_invoice_row(inv: &Value) -> String {
    let id = inv["invoiceid"].as_u64().unwrap_or(0);
    let number = inv["invoice_number"].as_str().unwrap_or("");
    let status_val = inv["v3_status"]
        .as_str()
        .or_else(|| inv["display_status"].as_str())
        .unwrap_or("");
    let amount = inv["amount"].as_object().map_or_else(
        || inv["amount"].as_str().unwrap_or("0").to_string(),
        |a| {
            format!(
                "{} {}",
                a.get("amount").and_then(|v| v.as_str()).unwrap_or("0"),
                a.get("code").and_then(|v| v.as_str()).unwrap_or("")
            )
        },
    );
    let customer = inv["current_organization"]
        .as_str()
        .unwrap_or_else(|| inv["fname"].as_str().unwrap_or(""));
    let date = inv["create_date"].as_str().unwrap_or("");
    format!("{id:<8} #{number:<8} {status_val:<12} {amount:<16} {customer:<24} {date}")
}

fn fmt_expense_row(e: &Value) -> String {
    let id = e["id"].as_u64().unwrap_or(0);
    let amount = e["amount"].as_object().map_or_else(
        || e["amount"].as_str().unwrap_or("0").to_string(),
        |a| {
            format!(
                "{} {}",
                a.get("amount").and_then(|v| v.as_str()).unwrap_or("0"),
                a.get("code").and_then(|v| v.as_str()).unwrap_or("")
            )
        },
    );
    let vendor = e["vendor"].as_str().unwrap_or("");
    let date = e["date"].as_str().unwrap_or("");
    let notes = e["notes"].as_str().unwrap_or("");
    format!("{id:<8} {amount:<16} {vendor:<24} {date:<12} {notes}")
}

fn fmt_payment_row(p: &Value) -> String {
    let id = p["id"].as_u64().unwrap_or(0);
    let amount = p["amount"].as_object().map_or_else(
        || p["amount"].as_str().unwrap_or("0").to_string(),
        |a| {
            format!(
                "{} {}",
                a.get("amount").and_then(|v| v.as_str()).unwrap_or("0"),
                a.get("code").and_then(|v| v.as_str()).unwrap_or("")
            )
        },
    );
    let inv_id = p["invoiceid"].as_u64().unwrap_or(0);
    let date = p["date"].as_str().unwrap_or("");
    let ptype = p["type"].as_str().unwrap_or("");
    format!("{id:<8} {amount:<16} invoice:{inv_id:<8} {date:<12} {ptype}")
}

/// Build a JSON object from optional fields, skipping None values.
macro_rules! json_object {
    ($($key:expr => $val:expr),* $(,)?) => {{
        let mut map = serde_json::Map::new();
        $(
            if let Some(ref v) = $val {
                map.insert($key.into(), serde_json::json!(v));
            }
        )*
        Value::Object(map)
    }};
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

#[tool_router]
impl FreshBooksServer {
    pub fn new(client: FreshBooksClient) -> Self {
        Self {
            client: std::sync::Arc::new(client),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Get the OAuth2 authorization URL. Open this URL in a browser to authorize, \
                        then use exchange_code with the code from the redirect URL."
    )]
    async fn get_auth_url(&self) -> Result<CallToolResult, McpError> {
        let url = self.client.auth_url();
        let text = format!(
            "Open this URL in your browser to authorize:\n\n{url}\n\n\
             After authorizing, your browser will redirect to a URL like:\n\
             https://localhost/callback?code=XXXX\n\n\
             Copy the 'code' parameter value and use the exchange_code tool with it."
        );
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "Exchange an OAuth2 authorization code for access tokens. \
                        Use after visiting the URL from get_auth_url."
    )]
    async fn exchange_code(
        &self,
        Parameters(p): Parameters<ExchangeCodeParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!("exchanging authorization code");
        match self.client.exchange_code(&p.code).await {
            Ok(_token) => {
                // Try to discover account_id.
                let account_msg = match self.client.account_id().await {
                    Ok(aid) => format!("Account ID: {aid}"),
                    Err(e) => format!("Could not auto-discover account_id: {e}"),
                };
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Authentication successful! Token saved.\n{account_msg}"
                ))]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Get authenticated user's account info and business memberships.")]
    async fn get_account_info(&self) -> Result<CallToolResult, McpError> {
        match self.client.get("/auth/api/v1/users/me").await {
            Ok(data) => {
                let pretty = serde_json::to_string_pretty(&data["response"])
                    .unwrap_or_else(|_| data.to_string());
                Ok(CallToolResult::success(vec![Content::text(pretty)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    // -----------------------------------------------------------------------
    // Clients
    // -----------------------------------------------------------------------

    #[tool(description = "List clients in FreshBooks. Returns paginated results.")]
    async fn list_clients(
        &self,
        Parameters(p): Parameters<ListParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let page = p.page.unwrap_or(1);
        let per_page = p.per_page.unwrap_or(25).min(100);
        let path = format!(
            "/accounting/account/{account_id}/users/clients?page={page}&per_page={per_page}"
        );

        match self.client.get(&path).await {
            Ok(data) => {
                let result = &data["response"]["result"];
                let total = result["total"].as_u64().unwrap_or(0);
                let pages = result["pages"].as_u64().unwrap_or(0);

                let mut out =
                    format!("Clients (page {page}/{pages}, {total} total):\n\n");
                out.push_str(&format!(
                    "{:<8} {:<21} {:<32} {}\n",
                    "ID", "Name", "Email", "Organization"
                ));
                out.push_str(&"-".repeat(90));
                out.push('\n');

                if let Some(clients) = result["clients"].as_array() {
                    for c in clients {
                        out.push_str(&fmt_client_row(c));
                        out.push('\n');
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(out)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Get a specific client by ID.")]
    async fn get_client(
        &self,
        Parameters(p): Parameters<GetByIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let path =
            format!("/accounting/account/{account_id}/users/clients/{}", p.id);

        match self.client.get(&path).await {
            Ok(data) => {
                let client = &data["response"]["result"]["client"];
                let pretty = serde_json::to_string_pretty(client)
                    .unwrap_or_else(|_| client.to_string());
                Ok(CallToolResult::success(vec![Content::text(pretty)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Search clients by keyword (searches name, email, organization).")]
    async fn search_clients(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let page = p.page.unwrap_or(1);
        let per_page = p.per_page.unwrap_or(25).min(100);
        let query = urlencoding(&p.query);
        let path = format!(
            "/accounting/account/{account_id}/users/clients\
             ?search[keyword]={query}&page={page}&per_page={per_page}"
        );

        match self.client.get(&path).await {
            Ok(data) => {
                let result = &data["response"]["result"];
                let total = result["total"].as_u64().unwrap_or(0);

                let mut out = format!("Search results ({total} found):\n\n");
                out.push_str(&format!(
                    "{:<8} {:<21} {:<32} {}\n",
                    "ID", "Name", "Email", "Organization"
                ));
                out.push_str(&"-".repeat(90));
                out.push('\n');

                if let Some(clients) = result["clients"].as_array() {
                    for c in clients {
                        out.push_str(&fmt_client_row(c));
                        out.push('\n');
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(out)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Create a new client. At least one of fname, lname, email, or organization is required.")]
    async fn create_client(
        &self,
        Parameters(p): Parameters<CreateClientParams>,
    ) -> Result<CallToolResult, McpError> {
        if p.fname.is_none()
            && p.lname.is_none()
            && p.email.is_none()
            && p.organization.is_none()
        {
            return Err(McpError::invalid_params(
                "At least one of fname, lname, email, or organization is required",
                None,
            ));
        }

        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let client_data = json_object! {
            "fname" => p.fname,
            "lname" => p.lname,
            "email" => p.email,
            "organization" => p.organization,
            "bus_phone" => p.bus_phone,
            "mob_phone" => p.mob_phone,
            "p_street" => p.p_street,
            "p_city" => p.p_city,
            "p_province" => p.p_province,
            "p_country" => p.p_country,
            "p_code" => p.p_code,
            "currency_code" => p.currency_code,
        };

        let body = serde_json::json!({ "client": client_data });
        let path =
            format!("/accounting/account/{account_id}/users/clients");

        match self.client.post(&path, &body).await {
            Ok(data) => {
                let client = &data["response"]["result"]["client"];
                let id = client["id"].as_u64().unwrap_or(0);
                let pretty = serde_json::to_string_pretty(client)
                    .unwrap_or_else(|_| client.to_string());
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Client created (ID: {id}):\n{pretty}"
                ))]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Update an existing client by ID.")]
    async fn update_client(
        &self,
        Parameters(p): Parameters<UpdateClientParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let client_data = json_object! {
            "fname" => p.fname,
            "lname" => p.lname,
            "email" => p.email,
            "organization" => p.organization,
            "bus_phone" => p.bus_phone,
            "mob_phone" => p.mob_phone,
            "p_street" => p.p_street,
            "p_city" => p.p_city,
            "p_province" => p.p_province,
            "p_country" => p.p_country,
            "p_code" => p.p_code,
            "currency_code" => p.currency_code,
        };

        let body = serde_json::json!({ "client": client_data });
        let path = format!(
            "/accounting/account/{account_id}/users/clients/{}",
            p.id
        );

        match self.client.put(&path, &body).await {
            Ok(data) => {
                let client = &data["response"]["result"]["client"];
                let pretty = serde_json::to_string_pretty(client)
                    .unwrap_or_else(|_| client.to_string());
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Client {} updated:\n{pretty}",
                    p.id
                ))]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    // -----------------------------------------------------------------------
    // Invoices
    // -----------------------------------------------------------------------

    #[tool(description = "List invoices. Returns paginated results.")]
    async fn list_invoices(
        &self,
        Parameters(p): Parameters<ListParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let page = p.page.unwrap_or(1);
        let per_page = p.per_page.unwrap_or(25).min(100);
        let path = format!(
            "/accounting/account/{account_id}/invoices/invoices?page={page}&per_page={per_page}"
        );

        match self.client.get(&path).await {
            Ok(data) => {
                let result = &data["response"]["result"];
                let total = result["total"].as_u64().unwrap_or(0);
                let pages = result["pages"].as_u64().unwrap_or(0);

                let mut out =
                    format!("Invoices (page {page}/{pages}, {total} total):\n\n");
                out.push_str(&format!(
                    "{:<8} {:<9} {:<12} {:<16} {:<24} {}\n",
                    "ID", "Number", "Status", "Amount", "Customer", "Date"
                ));
                out.push_str(&"-".repeat(95));
                out.push('\n');

                if let Some(invoices) = result["invoices"].as_array() {
                    for inv in invoices {
                        out.push_str(&fmt_invoice_row(inv));
                        out.push('\n');
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(out)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Get a specific invoice by ID.")]
    async fn get_invoice(
        &self,
        Parameters(p): Parameters<GetByIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let path = format!(
            "/accounting/account/{account_id}/invoices/invoices/{}",
            p.id
        );

        match self.client.get(&path).await {
            Ok(data) => {
                let invoice = &data["response"]["result"]["invoice"];
                let pretty = serde_json::to_string_pretty(invoice)
                    .unwrap_or_else(|_| invoice.to_string());
                Ok(CallToolResult::success(vec![Content::text(pretty)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(
        description = "Create a new invoice. Lines must be a JSON array of line items: \
                        [{\"name\": \"Service\", \"qty\": 1, \"unit_cost\": {\"amount\": \"100.00\", \"code\": \"USD\"}}]"
    )]
    async fn create_invoice(
        &self,
        Parameters(p): Parameters<CreateInvoiceParams>,
    ) -> Result<CallToolResult, McpError> {
        let lines: Value = serde_json::from_str(&p.lines).map_err(|e| {
            McpError::invalid_params(format!("Invalid lines JSON: {e}"), None)
        })?;

        if !lines.is_array() {
            return Err(McpError::invalid_params(
                "lines must be a JSON array",
                None,
            ));
        }

        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let mut invoice = serde_json::json!({
            "customerid": p.customerid,
            "lines": lines,
        });

        let obj = invoice.as_object_mut().unwrap();
        if let Some(ref s) = p.status {
            // FreshBooks uses numeric status: 1=draft, 2=sent
            let status_num = match s.as_str() {
                "draft" => 1,
                "sent" => 2,
                _ => 1,
            };
            obj.insert("status".into(), serde_json::json!(status_num));
        }
        if let Some(ref v) = p.notes {
            obj.insert("notes".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.create_date {
            obj.insert("create_date".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.due_date {
            obj.insert("due_date".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.currency_code {
            obj.insert("currency_code".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.po_number {
            obj.insert("po_number".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.discount_value {
            obj.insert("discount_value".into(), serde_json::json!(v));
        }

        let body = serde_json::json!({ "invoice": invoice });
        let path =
            format!("/accounting/account/{account_id}/invoices/invoices");

        match self.client.post(&path, &body).await {
            Ok(data) => {
                let inv = &data["response"]["result"]["invoice"];
                let id = inv["invoiceid"].as_u64().unwrap_or(0);
                let number = inv["invoice_number"].as_str().unwrap_or("?");
                let pretty = serde_json::to_string_pretty(inv)
                    .unwrap_or_else(|_| inv.to_string());
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Invoice created (ID: {id}, #{number}):\n{pretty}"
                ))]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    // -----------------------------------------------------------------------
    // Expenses
    // -----------------------------------------------------------------------

    #[tool(description = "List expenses. Returns paginated results.")]
    async fn list_expenses(
        &self,
        Parameters(p): Parameters<ListExpensesParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let page = p.page.unwrap_or(1);
        let per_page = p.per_page.unwrap_or(25).min(100);
        let path = format!(
            "/accounting/account/{account_id}/expenses/expenses?page={page}&per_page={per_page}"
        );

        match self.client.get(&path).await {
            Ok(data) => {
                let result = &data["response"]["result"];
                let total = result["total"].as_u64().unwrap_or(0);
                let pages = result["pages"].as_u64().unwrap_or(0);

                let mut out =
                    format!("Expenses (page {page}/{pages}, {total} total):\n\n");
                out.push_str(&format!(
                    "{:<8} {:<16} {:<24} {:<12} {}\n",
                    "ID", "Amount", "Vendor", "Date", "Notes"
                ));
                out.push_str(&"-".repeat(80));
                out.push('\n');

                if let Some(expenses) = result["expenses"].as_array() {
                    for e in expenses {
                        out.push_str(&fmt_expense_row(e));
                        out.push('\n');
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(out)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Create a new expense.")]
    async fn create_expense(
        &self,
        Parameters(p): Parameters<CreateExpenseParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let mut expense = serde_json::json!({
            "amount": {
                "amount": p.amount,
                "code": p.currency_code.as_deref().unwrap_or("USD"),
            }
        });

        let obj = expense.as_object_mut().unwrap();
        if let Some(ref v) = p.categoryid {
            obj.insert("categoryid".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.date {
            obj.insert("date".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.vendor {
            obj.insert("vendor".into(), serde_json::json!(v));
        }
        if let Some(ref v) = p.notes {
            obj.insert("notes".into(), serde_json::json!(v));
        }

        let body = serde_json::json!({ "expense": expense });
        let path =
            format!("/accounting/account/{account_id}/expenses/expenses");

        match self.client.post(&path, &body).await {
            Ok(data) => {
                let exp = &data["response"]["result"]["expense"];
                let id = exp["id"].as_u64().unwrap_or(0);
                let pretty = serde_json::to_string_pretty(exp)
                    .unwrap_or_else(|_| exp.to_string());
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Expense created (ID: {id}):\n{pretty}"
                ))]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    // -----------------------------------------------------------------------
    // Payments
    // -----------------------------------------------------------------------

    #[tool(description = "List payments. Returns paginated results.")]
    async fn list_payments(
        &self,
        Parameters(p): Parameters<ListPaymentsParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = self
            .client
            .account_id()
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let page = p.page.unwrap_or(1);
        let per_page = p.per_page.unwrap_or(25).min(100);
        let path = format!(
            "/accounting/account/{account_id}/payments/payments?page={page}&per_page={per_page}"
        );

        match self.client.get(&path).await {
            Ok(data) => {
                let result = &data["response"]["result"];
                let total = result["total"].as_u64().unwrap_or(0);
                let pages = result["pages"].as_u64().unwrap_or(0);

                let mut out =
                    format!("Payments (page {page}/{pages}, {total} total):\n\n");
                out.push_str(&format!(
                    "{:<8} {:<16} {:<16} {:<12} {}\n",
                    "ID", "Amount", "Invoice", "Date", "Type"
                ));
                out.push_str(&"-".repeat(70));
                out.push('\n');

                if let Some(payments) = result["payments"].as_array() {
                    for p in payments {
                        out.push_str(&fmt_payment_row(p));
                        out.push('\n');
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(out)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}

/// Simple URL encoding for query parameters.
fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// MCP ServerHandler
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for FreshBooksServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("freshbooks-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "FreshBooks accounting MCP server. Tools: get_auth_url, exchange_code, \
                 get_account_info, list_clients, get_client, search_clients, create_client, \
                 update_client, list_invoices, get_invoice, create_invoice, list_expenses, \
                 create_expense, list_payments.",
            )
    }
}
