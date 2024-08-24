use anyhow::{ Result, Context };
use neo4rs::{ Graph, Query, BoltMap, BoltType, BoltString, BoltList };
use tokio::sync::mpsc::Receiver;
use crate::{
    Thread,
    Message,
    Participant,
    THREAD_BATCH_SIZE,
    NEO4J_URI,
    NEO4J_USER,
    NEO4J_PASSWORD,
};
use log::{ info, warn };

pub async fn neo4j_writer(mut rx: Receiver<Thread>) -> Result<()> {
    info!("Starting Neo4j writer");
    let graph = Graph::new(NEO4J_URI, NEO4J_USER, NEO4J_PASSWORD).await.context(
        "Failed to connect to Neo4j"
    )?;
    info!("Connected to Neo4j successfully");
    let mut batch = Vec::new();
    let mut total_threads = 0;

    while let Some(thread) = rx.recv().await {
        batch.push(thread);
        total_threads += 1;

        if batch.len() >= THREAD_BATCH_SIZE {
            info!("Sending batch of {} threads to Neo4j", batch.len());
            send_batch_to_neo4j(&graph, &batch).await?;
            info!("Batch sent successfully. Total threads processed: {}", total_threads);
            batch.clear();
        }
    }

    // Send any remaining threads
    if !batch.is_empty() {
        info!("Sending final batch of {} threads to Neo4j", batch.len());
        send_batch_to_neo4j(&graph, &batch).await?;
        info!("Final batch sent successfully. Total threads processed: {}", total_threads);
    }

    info!("Neo4j writer completed. Total threads processed: {}", total_threads);
    Ok(())
}

async fn send_batch_to_neo4j(graph: &Graph, batch: &[Thread]) -> Result<()> {
    // Create participants
    info!("Creating participants");
    let participant_query = Query::new(
        "UNWIND $participants AS participant
         MERGE (p:Participant {phone: participant.phone})
         SET p.name = participant.name".to_string()
    ).param("participants", participant_to_params(batch));

    graph.run(participant_query).await?;
    info!("Creating messages and relationships");
    // Create messages and relationships
    let message_query = Query::new(
        "UNWIND $messages AS message
         MATCH (from:Participant {phone: message.from_phone})
         WITH message, from
         UNWIND message.to_phones AS to_phone
         MATCH (to:Participant {phone: to_phone})
         CREATE (m:Message {content: message.content, timestamp: message.timestamp})
         CREATE (from)-[:SENT]->(m)
         CREATE (m)-[:TO]->(to)".to_string()
    ).param("messages", message_to_params(batch));

    graph.run(message_query).await?;

    Ok(())
}

fn participant_to_params(batch: &[Thread]) -> BoltType {
    let mut participants = BoltList::new();
    for thread in batch {
        for p in &thread.participants {
            let mut map = BoltMap::new();
            map.put(BoltString::new("phone"), BoltType::String(BoltString::new(&p.phone)));
            map.put(BoltString::new("name"), BoltType::String(BoltString::new(&p.name)));
            participants.push(BoltType::Map(map));
        }
    }
    BoltType::List(participants)
}

fn message_to_params(batch: &[Thread]) -> BoltType {
    let mut messages = BoltList::new();
    for thread in batch {
        for m in &thread.messages {
            let mut map = BoltMap::new();
            map.put(
                BoltString::new("from_phone"),
                BoltType::String(BoltString::new(&m.from.phone))
            );
            let mut to_phones = BoltList::new();
            for to in &m.to {
                to_phones.push(BoltType::String(BoltString::new(&to.phone)));
            }
            map.put(BoltString::new("to_phones"), BoltType::List(to_phones));
            map.put(BoltString::new("content"), BoltType::String(BoltString::new(&m.content)));
            map.put(
                BoltString::new("timestamp"),
                BoltType::String(BoltString::new(&m.timestamp.to_rfc3339()))
            );
            messages.push(BoltType::Map(map));
        }
    }
    BoltType::List(messages)
}
