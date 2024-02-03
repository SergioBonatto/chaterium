#[tokio::main]
async fn main () {
    pretty_env_logger::init();
    
    info!("Peer Id: {}", p2p::PEER_ID.clone());
    let (reponse_sender, mut reponse_rcv) = mpsc::unbounded_channel();
    let (init_sender, mut init_rcv) = mpsc::unbounded_channel();

    let auth_keys = Keypair::::new()
        .into_authentic(&p2p::KEYS)
        .expect("can create auth keys");

    let transp = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let behaviour = p2p::AppBehaviour::new(transp, behaviour, *p2p::PEER_ID)
        .executor(Box::new(|fut| {
            spawn(fut);
        })) 
        .build();
    
    let mut stdin = BufReader::new(stdin()).lines();

    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("can get local socket"),
    )
        .expect("swarm can be started");
    
    Spawn(async move {
        sleep(Duration::from_secs(1)).await;
        info!("Sending init event");
        init_sender.send(true).expect("can send init event");
    })
    
    loop {
        let evt = {
            select! {
                line = stdin.next_line() => Some(p2p::EventType::Input(line.expect("can read line").expect("can read line from stdin"))),
                response = response_rcv.recv() => {
                    Some(p2p::EventType::LocalChainResponse(response.expect("response exists")))
                },
                _init = init_rcv.recv() => {
                    Some(p2p::EventType::Init)
                }
                event = swarm.select_next_some() => {
                    info!("Unhandled Swarm event: {:?}", event);
                    None
                },
            }
        };
        if let Some(event) = evt {
            match event {
                p2p::EventType::Init => {
                    let peers = p2p::get_peers(&swarm);
                    swarm.behaviour_mut().app.genesis();

                    info!("conected nodes: {}", peers.len());
                    if !peers.is_empty() {
                        let req = p2p::LocalChainRequest {
                            from_peer_id: peers
                                .iter()
                                .last()
                                .expect("at least on peer")
                                .to_string(),
                        };
                        let json = serde_json::to_string(&req).expect("can jsonify request");
                        swarm
                            .behaviour_mut()
                            .floodsub
                            .publish(CHAIN_TOPIC.clone(), json.as_bytes());
                    }
                }
                p2p::EventType::LocalChainResponse(resp) => {
                    let json = serde_json::to_string(&resp).expect("can jsonify response");
                    swarm
                        .behaviour_mut()
                        .floodsub
                        .publish(p2p::CHAIN_TOPIC.clone(), json.as_bytes());
                }
                p2p::EventType::Input(line) => match line.as_str() {
                    "ls p" => p2p::handle_print_peers(&swarm),
                    cmd if cmd.starts_with("ls c") => p2p::handle_print_chain(&swarm),
                    cmd if cmd.starts_with("creat b") => p2p::handle_create_block(cmd, &mut swarm),
                    _ => error!("Unknown command: {}", line),
                },
            }
        }
    }
   


    
}
