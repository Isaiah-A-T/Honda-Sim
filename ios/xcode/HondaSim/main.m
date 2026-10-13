// The app is written in Rust (shared/phone, ios); this hands it the main thread, where it starts UIKit.
extern void honda_sim_main(void);

int main(int argc, char *argv[]) {
    honda_sim_main();
    return 0;
}
