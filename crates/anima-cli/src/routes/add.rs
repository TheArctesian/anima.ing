//! `anima route add <name>`: adds a route of an application (a web name, a redirect, or a
//! stream of TCP or UDP; private unless `--access public`), and prints the TXT record that
//! proves the name. A login token only, with a second factor on the account
//! (SF-232). A stream needs a port of the hub (`--listen-port`) and, when public, a
//! protocol with its own sign-in and TLS (`--protocol`): the hub never picks a port by itself.

use super::documents::{RouteAdd, route_add};
use super::shown;
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct AddArgs {
    /// The DNS name, such as `photos.example.com`.
    name: String,
    /// The application's ID.
    #[arg(long)]
    application: String,
    /// The device's ID.
    #[arg(long)]
    device: String,
    /// The port of the application; not for a redirect. The route serves only when the hub's
    /// record of the application names this device and this port.
    #[arg(long)]
    port: Option<i64>,
    /// What the route carries.
    #[arg(long, value_enum, default_value = "web")]
    kind: KindArg,
    /// Who may reach it: only the devices of the network of Anima, or the internet.
    #[arg(long, value_enum, default_value = "private")]
    access: AccessArg,
    /// The port of the hub that a stream listens on (1024 or more, in the range of the hub).
    #[arg(long)]
    listen_port: Option<i64>,
    /// The protocol of a stream, needed when it is public: imaps (993), pop3s (995), smtps
    /// (465), ldaps (636), xmpps (5223) over TCP; openvpn (1194), wireguard (51820) over UDP.
    /// The usual port is a hint only.
    #[arg(long)]
    protocol: Option<String>,
    /// The one `https://` address of a redirect.
    #[arg(long)]
    redirect_to: Option<String>,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum KindArg {
    Web,
    Redirect,
    TcpStream,
    UdpStream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum AccessArg {
    Private,
    Public,
}

impl AddArgs {
    /// Asks for confirmation (`--yes` skips it), adds the route and prints it.
    ///
    /// # Errors
    /// [`Failure::Usage`] when it cannot ask; the hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let question = if self.access == AccessArg::Public {
            format!(
                "Make the application public on the internet as {}? It is then open to each person.",
                self.name
            )
        } else {
            format!(
                "Open {} to each node of the network of Anima: each device, machine and container on it?",
                self.name
            )
        };
        invocation.prompter.confirm(&question)?;
        let input = route_add::ApplicationRouteInput {
            name: self.name,
            application_id: self.application,
            device_id: self.device,
            port: self.port,
            kind: match self.kind {
                KindArg::Web => route_add::ApplicationRouteKind::WEB,
                KindArg::Redirect => route_add::ApplicationRouteKind::REDIRECT,
                KindArg::TcpStream => route_add::ApplicationRouteKind::TCP_STREAM,
                KindArg::UdpStream => route_add::ApplicationRouteKind::UDP_STREAM,
            },
            access: match self.access {
                AccessArg::Private => route_add::ApplicationRouteAccess::PRIVATE,
                AccessArg::Public => route_add::ApplicationRouteAccess::PUBLIC,
            },
            listen_port: self.listen_port,
            protocol: self.protocol,
            redirect_to: self.redirect_to,
        };
        let added = invocation
            .hub()?
            .run::<RouteAdd>(route_add::Variables { input })
            .await?
            .add_application_route;
        invocation.output.data(&added.route, shown::record)?;
        if added.same_site_as_hub {
            invocation.output.note(
                "The name shares its domain with the hub: a page of the application can send \
                 requests to the hub with its cookies.",
            );
        }
        Ok(())
    }
}
