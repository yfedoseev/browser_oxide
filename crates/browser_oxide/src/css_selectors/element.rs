/// Trait for DOM elements that can be matched against CSS selectors.
///
/// This trait is generic — implement it for any DOM tree representation.
/// The matching engine uses only these methods to traverse and query the tree.
pub trait Element: Sized + Clone {
    /// The element's local name (e.g., `"div"`, `"span"`).
    fn local_name(&self) -> &str;

    /// The element's namespace URI, if any.
    fn namespace(&self) -> Option<&str> {
        None
    }

    /// The element's ID attribute value, if any.
    fn id(&self) -> Option<&str>;

    /// Whether the element has the given class name.
    fn has_class(&self, name: &str) -> bool;

    /// Whether the element has an attribute with the given name.
    fn has_attribute(&self, name: &str) -> bool;

    /// The value of the attribute with the given name.
    fn attribute_value(&self, name: &str) -> Option<&str>;

    /// The parent element (not parent node — skips non-element parents).
    fn parent_element(&self) -> Option<Self>;

    /// The previous sibling element.
    fn prev_sibling_element(&self) -> Option<Self>;

    /// The next sibling element.
    fn next_sibling_element(&self) -> Option<Self>;

    /// The first child element.
    fn first_child_element(&self) -> Option<Self>;

    /// The last child element.
    fn last_child_element(&self) -> Option<Self>;

    /// Whether this is the root element of the document.
    fn is_root(&self) -> bool {
        self.parent_element().is_none()
    }

    /// Whether this element has no child elements and no text content.
    fn is_empty(&self) -> bool {
        self.first_child_element().is_none()
    }

    // --- Pseudo-class state (defaults to false) ---

    fn is_link(&self) -> bool {
        false
    }
    fn is_visited(&self) -> bool {
        false
    }
    fn is_hover(&self) -> bool {
        false
    }
    fn is_active(&self) -> bool {
        false
    }
    fn is_focus(&self) -> bool {
        false
    }
    fn is_focus_within(&self) -> bool {
        false
    }
    fn is_focus_visible(&self) -> bool {
        false
    }
    fn is_enabled(&self) -> bool {
        false
    }
    fn is_disabled(&self) -> bool {
        false
    }
    fn is_checked(&self) -> bool {
        false
    }
    fn is_target(&self) -> bool {
        false
    }
    fn is_read_write(&self) -> bool {
        false
    }
    fn is_read_only(&self) -> bool {
        !self.is_read_write()
    }
    fn is_required(&self) -> bool {
        false
    }
    fn is_optional(&self) -> bool {
        !self.is_required()
    }
    fn is_valid(&self) -> bool {
        true
    }
    fn is_invalid(&self) -> bool {
        !self.is_valid()
    }
    fn is_default(&self) -> bool {
        false
    }
    fn is_indeterminate(&self) -> bool {
        false
    }
    fn is_placeholder_shown(&self) -> bool {
        false
    }
    fn is_any_link(&self) -> bool {
        self.is_link() || self.is_visited()
    }
    fn is_in_range(&self) -> bool {
        false
    }
    fn is_out_of_range(&self) -> bool {
        false
    }
    fn lang(&self) -> Option<&str> {
        None
    }

    /// Iterate over all child elements.
    fn child_elements(&self) -> Vec<Self> {
        let mut children = Vec::new();
        let mut child = self.first_child_element();
        while let Some(c) = child {
            let next = c.next_sibling_element();
            children.push(c);
            child = next;
        }
        children
    }

    /// Count of preceding sibling elements + 1 (1-based index among siblings).
    fn sibling_index(&self) -> i32 {
        let mut index = 1;
        let mut sib = self.prev_sibling_element();
        while let Some(s) = sib {
            index += 1;
            sib = s.prev_sibling_element();
        }
        index
    }

    /// Count of following sibling elements + 1 (1-based index from end).
    fn sibling_index_from_end(&self) -> i32 {
        let mut index = 1;
        let mut sib = self.next_sibling_element();
        while let Some(s) = sib {
            index += 1;
            sib = s.next_sibling_element();
        }
        index
    }

    /// 1-based index among siblings of the same type.
    fn sibling_type_index(&self) -> i32 {
        let name = self.local_name().to_ascii_lowercase();
        let mut index = 1;
        let mut sib = self.prev_sibling_element();
        while let Some(s) = sib {
            if s.local_name().eq_ignore_ascii_case(&name) {
                index += 1;
            }
            sib = s.prev_sibling_element();
        }
        index
    }

    /// 1-based index from end among siblings of the same type.
    fn sibling_type_index_from_end(&self) -> i32 {
        let name = self.local_name().to_ascii_lowercase();
        let mut index = 1;
        let mut sib = self.next_sibling_element();
        while let Some(s) = sib {
            if s.local_name().eq_ignore_ascii_case(&name) {
                index += 1;
            }
            sib = s.next_sibling_element();
        }
        index
    }

    /// Total sibling count of the same type.
    fn sibling_type_count(&self) -> i32 {
        self.sibling_type_index() + self.sibling_type_index_from_end() - 1
    }
}
