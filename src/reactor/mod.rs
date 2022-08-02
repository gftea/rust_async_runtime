mod epoll;
// we do not need to export Reactor type
pub use epoll::Registry;
pub use epoll::enter;
pub use epoll::get_registery;