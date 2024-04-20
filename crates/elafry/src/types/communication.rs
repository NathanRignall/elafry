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

#[cfg(test)]
mod tests {
    use super::*;

    // setup logging
    fn setup() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("warn,info,debug,trace"),
        )
        .is_test(true)
        .try_init();
    }

    #[test]
    fn test_encode_decode() {
        setup();

        let message = Message {
            channel_id: 1,
            count: 2,
            data: vec![3, 4, 5],
        };

        let encoded = message.encode();
        let decoded = Message::decode(&encoded).unwrap();

        assert_eq!(message, decoded);
    }

    #[test]
    fn test_decode_empty() {
        setup();

        let data = vec![];
        let decoded = Message::decode(&data);

        assert_eq!(decoded, None);
    }

    #[test]
    fn test_decode_short() {
        setup();

        let data = vec![1, 2, 3, 4];
        let decoded = Message::decode(&data);

        assert_eq!(decoded, None);
    }

    #[test]
    fn test_decode() {
        setup();

        let data = vec![0, 0, 0, 1, 5, 6, 7, 8];
        let decoded = Message::decode(&data).unwrap();

        assert_eq!(decoded.channel_id, 1);
        assert_eq!(decoded.count, 5);
        assert_eq!(decoded.data, vec![6, 7, 8]);
    }

    #[test]
    fn test_encode() {
        setup();

        let message = Message {
            channel_id: 1,
            count: 2,
            data: vec![3, 4, 5],
        };

        let encoded = message.encode();

        assert_eq!(encoded, vec![0, 0, 0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_type() {
        setup();
        
        let message = Message {
            channel_id: 1,
            count: 2,
            data: vec![3, 4, 5],
        };

        // test eq
        assert_eq!(message, message.clone());

        // test partial eq
        assert_eq!(message, Message {
            channel_id: 1,
            count: 2,
            data: vec![3, 4, 5],
        });

        // test debug
        assert_eq!(format!("{:?}", message), "Message { channel_id: 1, count: 2, data: [3, 4, 5] }");

        assert_eq!(message.channel_id, 1);
        assert_eq!(message.count, 2);
        assert_eq!(message.data, vec![3, 4, 5]);
    }
}
