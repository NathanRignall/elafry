#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Message {
    pub channel_id: u32,
    pub count: u8,
    pub data: Vec<u8>,
}

impl Message {
    pub fn decode(data: &Vec<u8>) -> Option<Message> {
        if data.len() < 5 {
            return None;
        }

        // first 4 bytes are channel id
        let channel_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        // next byte is count
        let count = data[4];

        // rest of data is message
        let data = data[5..].to_vec();

        Some(Message {
            channel_id,
            count,
            data,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut data = vec![];

        data.extend_from_slice(&self.channel_id.to_be_bytes());
        data.push(self.count);
        data.extend_from_slice(&self.data);

        data
    }
    
}
