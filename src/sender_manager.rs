#[derive(Debug, Clone)]
pub enum Item<T> {
    Sender(T),
    Empty,
}

#[derive(Debug, Clone)]
pub struct SenderManager<T> {
    senders: Vec<Item<T>>,
    next: usize,
}

impl<T> SenderManager<T> {
    pub fn new() -> Self {
        SenderManager {
            senders: vec![],
            next: 0,
        }
    }

    pub fn add(&mut self, item: T) -> usize {
        let idx = self.next;
        self.senders.insert(idx, Item::Sender(item));
        self.update_next();

        idx
    }

    pub fn remove(&mut self, idx: usize) -> Option<T> {
        // Check if index is out of bounds.
        if idx >= self.senders.len() {
            return None;
        }

        let item = self.senders.remove(idx);
        self.senders.insert(idx, Item::Empty);

        if self.next > idx {
            self.next = idx;
        }

        item.into()
    }

    // pub fn collect_senders(&self) -> &Vec<&T> {
    //     &self
    //         .senders
    //         .iter()
    //         .filter(|s| !s.is_empty())
    //         .map(|s| &s.unwrap())
    //         .collect::<Vec<&T>>()
    // }

    fn get_senders(&self) -> &Vec<Item<T>> {
        &self.senders
    }

    fn update_next(&mut self) {
        while !matches!(self.senders.get(self.next), None)
            && !matches!(self.senders.get(self.next), Some(Item::Empty))
        {
            self.next += 1;
        }
    }
}

impl<T> Item<T> {
    pub fn is_empty(&self) -> bool {
        if let Item::Empty = self {
            return true;
        }

        false
    }

    pub fn unwrap(self) -> T {
        match self {
            Item::Sender(s) => s,
            Item::Empty => panic!("Cannot unwrap Item::Empty"),
        }
    }
}

impl<T> From<Option<T>> for Item<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => Item::Sender(v),
            None => Item::Empty,
        }
    }
}

impl<T> From<Item<T>> for Option<T> {
    fn from(value: Item<T>) -> Self {
        match value {
            Item::Sender(v) => Some(v),
            Item::Empty => None,
        }
    }
}

impl<T> PartialEq for Item<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match self {
            Item::Sender(s) => {
                if let Item::Sender(o) = other {
                    return s == o;
                }
            }
            Item::Empty => {
                if let Item::Empty = other {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use crate::sender_manager::{Item, SenderManager};

    #[test]
    fn check_add() {
        let num1 = 1;
        let num2 = 2;
        let mut manager = SenderManager::new();
        let idx1 = manager.add(num1);
        let idx2 = manager.add(num2);
        let senders_ref = manager.get_senders();

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(senders_ref.len(), 2);
        assert_eq!(senders_ref.get(idx1), Some(&Item::Sender(num1)));
        assert_eq!(senders_ref.get(idx2), Some(&Item::Sender(num2)));
    }

    #[test]
    fn check_remove() {
        let num1 = 1;
        let num2 = 2;
        let mut manager = SenderManager::new();
        let idx1 = manager.add(num1);
        let idx2 = manager.add(num2);

        assert_eq!(manager.get_senders().len(), 2);

        let removed1 = manager.remove(idx1);
        assert_eq!(removed1, Some(num1));
        assert_eq!(manager.get_senders().len(), 2);
        assert_eq!(manager.get_senders().get(idx1), Some(&Item::Empty));

        let removed2 = manager.remove(idx2);
        assert_eq!(removed2, Some(num2));
        assert_eq!(manager.get_senders().len(), 2);
        assert_eq!(manager.get_senders().get(idx2), Some(&Item::Empty));
    }

    #[test]
    fn check_invalid_remove() {
        let num1 = 1;
        let mut manager = SenderManager::new();
        let idx1 = manager.add(num1);

        assert_eq!(manager.get_senders().len(), 1);

        let removed = manager.remove(1);
        assert_eq!(removed, None);
        assert_eq!(manager.get_senders().len(), 1);
        assert_eq!(manager.get_senders(), &vec![Item::Sender(num1)]);
    }

    #[test]
    fn check_readd() {
        let num1 = 1;
        let num2 = 2;
        let mut manager = SenderManager::new();
        let idx1 = manager.add(num1);
        let idx2 = manager.add(num2);

        assert_eq!(manager.get_senders().len(), 2);
        assert_eq!(idx1, 0);

        manager.remove(idx1);
        let idx1 = manager.add(num1);
        assert_eq!(idx1, 0);
    }

    #[test]
    fn check_item_eq_empty() {
        let item1 = Item::<()>::Empty;
        let item2 = Item::<()>::Empty;

        assert_eq!(item1, item2);
    }

    #[test]
    fn check_item_eq_sender() {
        let item1 = Item::Sender(1);
        let item2 = Item::Sender(1);

        assert_eq!(item1, item2);
    }

    #[test]
    fn check_item_ne() {
        let item1 = Item::<i32>::Empty;
        let item2 = Item::<i32>::Sender(1);

        assert_ne!(item1, item2);
    }

    #[test]
    fn check_item_ne_sender() {
        let item1 = Item::Sender(1);
        let item2 = Item::Sender(2);

        assert_ne!(item1, item2);
    }
}
